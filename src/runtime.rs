use std::io::{self, IsTerminal, Write};
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
    let mut driver = RenderDriver::new(config, io::stdout().is_terminal());

    let stdout = io::stdout();
    let mut writer = stdout.lock();

    let mut interrupting_engine =
        InterruptingEngine::new(engine, signals.interrupt_flag(), INTERRUPT_POLL_SLICE);

    let run_result = app::run_with_observer(&mut interrupting_engine, app_config, |event| {
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
    flushed_len: usize,
}

impl RenderDriver {
    #[must_use]
    pub fn new(config: &Config, stdout_is_terminal: bool) -> Self {
        let use_terminal = config.terminal.unwrap_or(stdout_is_terminal);
        let mut render_config = RenderConfig::from(config);

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
                flushed_len: 0,
            };
        }

        Self {
            renderer: Renderer::Plain(PlainRenderer::new(render_config)),
            flushed_len: 0,
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
        let Some((columns, lines)) = terminal_dimensions() else {
            return;
        };

        self.renderer.update_size(columns, lines);
    }

    fn flush_incremental_output(&mut self, writer: &mut impl Write) -> io::Result<()> {
        let output = self.renderer.output();

        if self.flushed_len > output.len() {
            self.flushed_len = 0;
        }

        if self.flushed_len < output.len() {
            let bytes = output.as_bytes();
            writer.write_all(&bytes[self.flushed_len..])?;
            self.flushed_len = output.len();
        }

        writer.flush()
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

    fn output(&self) -> &str {
        match self {
            Self::Plain(renderer) => renderer.output(),
            Self::Terminal(renderer) => renderer.output(),
        }
    }

    fn update_size(&mut self, columns: u16, lines: u16) {
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
}
