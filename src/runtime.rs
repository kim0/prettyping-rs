use std::io::{self, IsTerminal, Write};
use std::num::NonZeroU16;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use crate::app::{self, AppConfig, AppError, AppEvent, AppReport};
use crate::config::Config;
use crate::engine::{EngineTime, PingEngine, PingEngineError, PingEvent, TimedEvent};
use crate::render::plain::PlainRenderer;
use crate::render::terminal::TerminalRenderer;
use crate::render::RenderConfig;

const INTERRUPT_POLL_SLICE: Duration = Duration::from_millis(50);

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("failed to install signal handlers: {message}")]
    SignalInstall { message: String },
    #[error("failed to flush output: {message}")]
    Output { message: String },
    #[error(transparent)]
    App(#[from] AppError),
}

pub fn run_with_runtime<E>(
    engine: &mut E,
    app_config: &AppConfig,
    config: &Config,
) -> Result<AppReport, RuntimeError>
where
    E: PingEngine,
{
    let signals = RuntimeSignals::install()?;
    let stdout_is_terminal = io::stdout().is_terminal();

    #[cfg(target_os = "windows")]
    {
        let use_terminal = config.terminal.unwrap_or(stdout_is_terminal);
        // Best-effort enabling of ANSI/VT escape processing for classic Windows console hosts.
        // If this fails (e.g., redirected output), we continue and the user may see raw escapes.
        if use_terminal {
            let _ = crate::windows_console::enable_virtual_terminal_processing();
        }
    }

    let mut driver = RenderDriver::new(config, stdout_is_terminal);

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let mut interrupting_engine =
        InterruptingEngine::new(engine, signals.interrupt_flag(), INTERRUPT_POLL_SLICE);

    // Streaming mode: do not retain an unbounded event log in memory.
    let run_result =
        app::run_streaming_with_observer(&mut interrupting_engine, app_config, |event| {
            let resize_requested = signals.take_resize_requested();
            driver
                .observe_event(event, resize_requested, &mut writer)
                .map_err(|err| AppError::Observer {
                    message: err.to_string(),
                })
        });

    let finish_result = driver.finish(&mut writer);

    let report = run_result.map_err(RuntimeError::from)?;

    if let Err(err) = finish_result {
        return Err(RuntimeError::Output {
            message: err.to_string(),
        });
    }

    Ok(report)
}

pub struct InterruptingEngine<'a, E> {
    inner: &'a mut E,
    interrupt_requested: &'a AtomicBool,
    poll_slice: Duration,
}

impl<'a, E> InterruptingEngine<'a, E>
where
    E: PingEngine,
{
    #[must_use]
    pub fn new(
        inner: &'a mut E,
        interrupt_requested: &'a AtomicBool,
        poll_slice: Duration,
    ) -> Self {
        Self {
            inner,
            interrupt_requested,
            poll_slice,
        }
    }

    fn interrupt_is_requested(&self) -> bool {
        self.interrupt_requested.load(Ordering::SeqCst)
    }

    fn interrupt_event(&self, at: EngineTime) -> TimedEvent {
        TimedEvent {
            at,
            event: PingEvent::Interrupt,
        }
    }
}

impl<E> PingEngine for InterruptingEngine<'_, E>
where
    E: PingEngine,
{
    fn now(&self) -> EngineTime {
        self.inner.now()
    }

    fn send_probe(&mut self, request: crate::engine::ProbeRequest) -> Result<(), PingEngineError> {
        self.inner.send_probe(request)
    }

    fn poll_until(&mut self, deadline: EngineTime) -> Result<Vec<TimedEvent>, PingEngineError> {
        if deadline < self.inner.now() {
            return Err(PingEngineError::NonMonotonicPoll);
        }

        loop {
            let now = self.inner.now();

            if self.interrupt_is_requested() {
                let immediate_events = self.inner.poll_until(now)?;
                if !immediate_events.is_empty() {
                    return Ok(immediate_events);
                }

                return Ok(vec![self.interrupt_event(now)]);
            }

            if now >= deadline {
                return Ok(Vec::new());
            }

            let sliced_deadline = now
                .checked_add(self.poll_slice)
                .map_or(deadline, |at| at.min(deadline));

            let events = self.inner.poll_until(sliced_deadline)?;
            if !events.is_empty() {
                return Ok(events);
            }

            if self.interrupt_is_requested() {
                return Ok(vec![self.interrupt_event(self.inner.now())]);
            }

            if self.inner.now() >= deadline {
                return Ok(Vec::new());
            }
        }
    }
}

#[derive(Debug)]
pub struct RenderDriver {
    renderer: Renderer,
    terminal_columns_locked: bool,
    terminal_lines_locked: bool,
}

impl RenderDriver {
    #[must_use]
    pub fn new(config: &Config, stdout_is_terminal: bool) -> Self {
        let use_terminal = config.terminal.unwrap_or(stdout_is_terminal);
        let mut render_config = RenderConfig::from(config);
        let terminal_columns_locked = render_config.columns.is_some();
        let terminal_lines_locked = render_config.lines.is_some();

        if use_terminal {
            if let Some((columns, lines)) = terminal_dimensions() {
                if render_config.columns.is_none() {
                    render_config.columns = Some(columns);
                }
                if render_config.lines.is_none() {
                    render_config.lines = Some(lines);
                }
            }

            return Self {
                renderer: Renderer::Terminal(TerminalRenderer::new(render_config)),
                terminal_columns_locked,
                terminal_lines_locked,
            };
        }

        Self {
            renderer: Renderer::Plain(PlainRenderer::new(render_config)),
            terminal_columns_locked,
            terminal_lines_locked,
        }
    }

    pub fn observe_event(
        &mut self,
        event: &AppEvent,
        resize_requested: bool,
        writer: &mut impl Write,
    ) -> io::Result<()> {
        if resize_requested {
            self.update_terminal_size();
        }

        self.renderer.render_event(event);
        self.flush_incremental_output(writer)
    }

    pub fn finish(&mut self, writer: &mut impl Write) -> io::Result<()> {
        self.renderer.finish();
        self.flush_incremental_output(writer)?;

        if self.renderer.is_terminal() {
            writer.write_all(b"\x1b[0m\n")?;
        }

        writer.flush()
    }

    fn update_terminal_size(&mut self) {
        self.update_terminal_size_with(terminal_dimensions());
    }

    fn update_terminal_size_with(&mut self, dimensions: Option<(u16, u16)>) {
        let Some((columns, lines)) = dimensions else {
            return;
        };

        let effective_columns = if self.terminal_columns_locked {
            None
        } else {
            NonZeroU16::new(columns).map(NonZeroU16::get)
        };

        let effective_lines = if self.terminal_lines_locked {
            None
        } else {
            NonZeroU16::new(lines).map(NonZeroU16::get)
        };

        self.renderer
            .update_size(effective_columns, effective_lines);
    }

    fn flush_incremental_output(&mut self, writer: &mut impl Write) -> io::Result<()> {
        let output = self.renderer.output_mut();
        if !output.is_empty() {
            writer.write_all(output.as_bytes())?;
            output.clear();
        }

        writer.flush()
    }

    #[cfg(test)]
    fn flush_incremental_output_for_test(&mut self, writer: &mut impl Write) -> io::Result<()> {
        self.flush_incremental_output(writer)
    }
}

#[derive(Debug)]
enum Renderer {
    Plain(PlainRenderer),
    Terminal(TerminalRenderer),
}

impl Renderer {
    fn render_event(&mut self, event: &AppEvent) {
        match self {
            Self::Plain(renderer) => renderer.render_event(event),
            Self::Terminal(renderer) => renderer.render_event(event),
        }
    }

    fn finish(&mut self) {
        match self {
            Self::Plain(renderer) => renderer.finish(),
            Self::Terminal(renderer) => renderer.finish(),
        }
    }

    fn output_mut(&mut self) -> &mut String {
        match self {
            Self::Plain(renderer) => renderer.output_mut(),
            Self::Terminal(renderer) => renderer.output_mut(),
        }
    }

    fn update_size(&mut self, columns: Option<u16>, lines: Option<u16>) {
        if let Self::Terminal(renderer) = self {
            renderer.update_size(columns, lines);
        }
    }

    fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminal(_))
    }
}

fn terminal_dimensions() -> Option<(u16, u16)> {
    let (terminal_size::Width(columns), terminal_size::Height(lines)) =
        terminal_size::terminal_size()?;
    Some((columns, lines))
}

pub struct RuntimeSignals {
    interrupt_requested: Arc<AtomicBool>,
    resize_requested: Arc<AtomicBool>,
    _hooks: SignalHooks,
}

impl RuntimeSignals {
    fn install() -> Result<Self, RuntimeError> {
        let interrupt_requested = Arc::new(AtomicBool::new(false));
        let resize_requested = Arc::new(AtomicBool::new(false));

        let hooks = SignalHooks::install(
            Arc::clone(&interrupt_requested),
            Arc::clone(&resize_requested),
        )
        .map_err(|err| RuntimeError::SignalInstall {
            message: err.to_string(),
        })?;

        Ok(Self {
            interrupt_requested,
            resize_requested,
            _hooks: hooks,
        })
    }

    fn interrupt_flag(&self) -> &AtomicBool {
        &self.interrupt_requested
    }

    fn take_resize_requested(&self) -> bool {
        self.resize_requested.swap(false, Ordering::SeqCst)
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
struct SignalHooks {
    ids: Vec<signal_hook::SigId>,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl SignalHooks {
    fn install(interrupt: Arc<AtomicBool>, resize: Arc<AtomicBool>) -> io::Result<Self> {
        let ids = vec![
            signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&interrupt))?,
            signal_hook::flag::register(signal_hook::consts::SIGTERM, interrupt)?,
            signal_hook::flag::register(signal_hook::consts::SIGWINCH, resize)?,
        ];

        Ok(Self { ids })
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for SignalHooks {
    fn drop(&mut self) {
        for id in self.ids.drain(..) {
            signal_hook::low_level::unregister(id);
        }
    }
}

#[cfg(target_os = "windows")]
struct SignalHooks;

#[cfg(target_os = "windows")]
impl SignalHooks {
    fn install(interrupt: Arc<AtomicBool>, _resize: Arc<AtomicBool>) -> io::Result<Self> {
        ctrlc::set_handler(move || {
            interrupt.store(true, Ordering::SeqCst);
        })
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err.to_string()))?;

        Ok(Self)
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
struct SignalHooks;

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
impl SignalHooks {
    fn install(_interrupt: Arc<AtomicBool>, _resize: Arc<AtomicBool>) -> io::Result<Self> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::net::{IpAddr, Ipv4Addr};
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    use crate::app::{run_with_observer, AppConfig, AppEvent};
    use crate::engine::mock::MockEngine;
    use crate::engine::{PingEngine, PingEvent, PingReply, TimedEvent};

    use super::{InterruptingEngine, RenderDriver};
    use crate::config::{AddressFamily, Config};

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    fn base_app_config(count: Option<u64>) -> AppConfig {
        AppConfig {
            target: IpAddr::V4(Ipv4Addr::LOCALHOST),
            interval: ms(1_000),
            timeout: ms(2_000),
            count,
            payload_size: 56,
            ttl: None,
        }
    }

    fn base_render_config() -> Config {
        Config {
            host: "127.0.0.1".to_string(),
            color: false,
            multicolor: false,
            unicode: false,
            legend: true,
            globalstats: true,
            recentstats: true,
            terminal: Some(false),
            last: 10,
            columns: Some(80),
            lines: Some(24),
            rttmin: None,
            rttmax: None,
            family: AddressFamily::Any,
            count: Some(10),
            interval_secs: Some(1.0),
            timeout_secs: Some(1.0),
            packet_size: Some(56),
            ttl: Some(64),
        }
    }

    #[test]
    fn interrupting_engine_stops_run_promptly_when_interrupt_flag_is_raised() {
        let interrupt_requested = AtomicBool::new(false);
        let mut engine = MockEngine::with_now(Duration::ZERO);

        engine.queue_event(TimedEvent {
            at: ms(200),
            event: PingEvent::Reply(PingReply::for_seq(1)),
        });

        let mut interrupting_engine =
            InterruptingEngine::new(&mut engine, &interrupt_requested, ms(50));

        let mut driver = RenderDriver::new(&base_render_config(), false);
        let mut output = Vec::new();

        let report = run_with_observer(&mut interrupting_engine, &base_app_config(None), |event| {
            driver
                .observe_event(event, false, &mut output)
                .map_err(|err| crate::app::AppError::Observer {
                    message: err.to_string(),
                })?;

            if matches!(event, AppEvent::ProbeReply { .. }) {
                interrupt_requested.store(true, std::sync::atomic::Ordering::SeqCst);
            }

            Ok(())
        })
        .expect("runtime should exit on synthetic interrupt");

        driver
            .finish(&mut output)
            .expect("finish should flush output");

        assert!(report.interrupted);
        assert!(report
            .events
            .iter()
            .any(|event| matches!(event, AppEvent::Interrupted { .. })));

        let sent_sequences: Vec<u64> = engine
            .sent_requests()
            .iter()
            .map(|request| request.seq)
            .collect();
        assert_eq!(sent_sequences, vec![1]);
    }

    #[test]
    fn terminal_finish_always_appends_reset_and_newline() {
        let mut config = base_render_config();
        config.terminal = Some(true);

        let mut driver = RenderDriver::new(&config, true);
        let mut output = Vec::new();

        driver
            .observe_event(
                &AppEvent::ProbeReply {
                    seq: 1,
                    sent_at: Duration::ZERO,
                    received_at: ms(10),
                    rtt_ms: 10,
                    duplicate: false,
                    late: false,
                },
                false,
                &mut output,
            )
            .expect("event rendering should succeed");

        driver.finish(&mut output).expect("finish should succeed");

        let text = String::from_utf8(output).expect("output should be utf8");
        assert!(text.ends_with("\x1b[0m\n"));
        assert!(
            text.ends_with("\n\n\n\x1b[0m\n"),
            "terminal finish should move below reserved stats lines before final reset"
        );
    }

    #[test]
    fn interrupting_engine_defers_interrupt_when_ready_events_exist() {
        let interrupt_requested = AtomicBool::new(true);
        let mut engine = MockEngine::with_now(Duration::ZERO);
        engine.queue_event(TimedEvent {
            at: Duration::ZERO,
            event: PingEvent::Reply(PingReply::for_seq(7)),
        });

        let mut interrupting_engine =
            InterruptingEngine::new(&mut engine, &interrupt_requested, ms(50));

        let events = interrupting_engine
            .poll_until(ms(100))
            .expect("poll should return immediate reply");

        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0].event,
            PingEvent::Reply(PingReply { seq: 7, .. })
        ));
    }

    #[test]
    fn render_driver_handles_resize_hint_without_terminal_dimensions() {
        let mut config = base_render_config();
        config.terminal = Some(true);

        let mut driver = RenderDriver::new(&config, true);
        let mut output = io::sink();

        driver
            .observe_event(
                &AppEvent::ProbeTimeout {
                    seq: 1,
                    sent_at: Duration::ZERO,
                    deadline: ms(900),
                },
                true,
                &mut output,
            )
            .expect("resize hint should not fail");
    }

    #[test]
    fn render_driver_clears_renderer_buffer_after_flush() {
        let config = base_render_config();
        let mut driver = RenderDriver::new(&config, false);
        let mut output = Vec::new();

        driver
            .observe_event(
                &AppEvent::ProbeTimeout {
                    seq: 1,
                    sent_at: Duration::ZERO,
                    deadline: ms(900),
                },
                false,
                &mut output,
            )
            .expect("render should succeed");

        // If the buffer is not cleared, a second flush would write the same bytes again.
        let first_len = output.len();
        driver
            .flush_incremental_output_for_test(&mut output)
            .expect("flush should succeed");
        assert_eq!(output.len(), first_len);
    }

    #[test]
    fn render_driver_resize_does_not_override_manual_columns_setting() {
        let mut config = base_render_config();
        config.terminal = Some(true);
        config.columns = Some(80);
        config.lines = None;

        let mut driver = RenderDriver::new(&config, true);

        // Render a few events to produce output at the configured width.
        let mut output = Vec::new();
        for seq in 1..=90 {
            driver
                .observe_event(
                    &AppEvent::ProbeTimeout {
                        seq,
                        sent_at: Duration::ZERO,
                        deadline: ms(900),
                    },
                    false,
                    &mut output,
                )
                .expect("render should succeed");
        }

        // Resize should NOT change columns when user set --columns.
        driver.update_terminal_size_with(Some((120, 40)));

        // After resizing, rendering should still wrap at around 80 columns, not 120.
        let before = String::from_utf8_lossy(&output).to_string();
        let before_max_line = before.lines().map(|l| l.len()).max().unwrap_or(0);

        output.clear();
        for seq in 91..=180 {
            driver
                .observe_event(
                    &AppEvent::ProbeTimeout {
                        seq,
                        sent_at: Duration::ZERO,
                        deadline: ms(900),
                    },
                    false,
                    &mut output,
                )
                .expect("render should succeed");
        }

        let after = String::from_utf8_lossy(&output).to_string();
        let after_max_line = after.lines().map(|l| l.len()).max().unwrap_or(0);

        // Allow some slack for terminal control sequences and legend/stats lines.
        assert!(before_max_line <= 200);
        assert!(after_max_line <= 200);
        assert!(after_max_line < 300);

        // The key property: it should not become substantially wider after resize.
        assert!(after_max_line <= before_max_line.saturating_add(40));
    }
}
