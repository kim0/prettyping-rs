use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use prettyping_rs::app::AppEvent;
use prettyping_rs::render::plain::PlainRenderer;
use prettyping_rs::render::terminal::TerminalRenderer;
use prettyping_rs::render::RenderConfig;

fn base_events() -> Vec<AppEvent> {
    vec![
        AppEvent::ProbeSent {
            seq: 1,
            at: Duration::ZERO,
        },
        AppEvent::ProbeReply {
            seq: 1,
            sent_at: Duration::ZERO,
            received_at: Duration::from_millis(12),
            rtt_ms: 12,
            duplicate: false,
            late: false,
        },
        AppEvent::ProbeSent {
            seq: 2,
            at: Duration::from_millis(1_000),
        },
        AppEvent::ProbeReply {
            seq: 2,
            sent_at: Duration::from_millis(1_000),
            received_at: Duration::from_millis(1_045),
            rtt_ms: 45,
            duplicate: false,
            late: false,
        },
        AppEvent::ProbeSent {
            seq: 3,
            at: Duration::from_millis(2_000),
        },
        AppEvent::ProbeTimeout {
            seq: 3,
            sent_at: Duration::from_millis(2_000),
            deadline: Duration::from_millis(2_900),
        },
        AppEvent::ProbeSent {
            seq: 4,
            at: Duration::from_millis(3_000),
        },
        AppEvent::ProbeReply {
            seq: 4,
            sent_at: Duration::from_millis(3_000),
            received_at: Duration::from_millis(3_210),
            rtt_ms: 210,
            duplicate: false,
            late: false,
        },
        AppEvent::ProbeSent {
            seq: 5,
            at: Duration::from_millis(4_000),
        },
        AppEvent::ProbeReply {
            seq: 5,
            sent_at: Duration::from_millis(4_000),
            received_at: Duration::from_millis(4_080),
            rtt_ms: 80,
            duplicate: false,
            late: false,
        },
        AppEvent::ProbeSent {
            seq: 6,
            at: Duration::from_millis(5_000),
        },
        AppEvent::ProbeTimeout {
            seq: 6,
            sent_at: Duration::from_millis(5_000),
            deadline: Duration::from_millis(5_900),
        },
    ]
}

fn base_render_config() -> RenderConfig {
    RenderConfig {
        color: false,
        multicolor: false,
        unicode: false,
        legend: true,
        globalstats: true,
        recentstats: true,
        last: 4,
        columns: Some(80),
        lines: Some(24),
        rttmin: None,
        rttmax: None,
    }
}

#[test]
fn plain_ascii_snapshot() {
    let mut renderer = PlainRenderer::new(base_render_config());
    for event in base_events() {
        renderer.render_event(&event);
    }
    renderer.finish();

    assert_snapshot(
        "plain_ascii.snapshot",
        &to_ascii_snapshot(renderer.output()),
    );
}

#[test]
fn terminal_ascii_multicolor_snapshot() {
    let mut config = base_render_config();
    config.color = true;
    config.multicolor = true;
    config.columns = Some(90);

    let mut renderer = TerminalRenderer::new(config);
    for event in base_events() {
        renderer.render_event(&event);
    }
    renderer.finish();

    assert_snapshot(
        "terminal_ascii_multicolor.snapshot",
        &to_ascii_snapshot(renderer.output()),
    );
}

#[test]
fn terminal_narrow_resize_snapshot() {
    let mut config = base_render_config();
    config.columns = Some(22);
    config.lines = Some(8);

    let mut renderer = TerminalRenderer::new(config);
    let events = base_events();

    for (index, event) in events.iter().enumerate() {
        if index == 7 {
            renderer.update_size(Some(16), Some(8));
        }
        renderer.render_event(event);
    }
    renderer.finish();

    assert_snapshot(
        "terminal_narrow_resize.snapshot",
        &to_ascii_snapshot(renderer.output()),
    );
}

#[test]
fn plain_unicode_snapshot_is_ascii_escaped() {
    let mut config = base_render_config();
    config.unicode = true;

    let mut renderer = PlainRenderer::new(config);
    for event in base_events() {
        renderer.render_event(&event);
    }
    renderer.finish();

    assert_snapshot(
        "plain_unicode_escaped.snapshot",
        &to_ascii_snapshot(renderer.output()),
    );
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = snapshot_path(name);

    if std::env::var("UPDATE_SNAPSHOTS").ok().as_deref() == Some("1") {
        fs::write(&path, actual).expect("snapshot write should succeed");
    }

    let expected = fs::read_to_string(&path).expect("snapshot file should exist");
    assert_eq!(actual, expected, "snapshot mismatch: {}", path.display());
}

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
        .join(name)
}

fn to_ascii_snapshot(input: &str) -> String {
    let mut out = String::new();

    for ch in input.chars() {
        match ch {
            '\x1b' => out.push_str("<ESC>"),
            '\n' => out.push('\n'),
            '\r' => out.push_str("<CR>"),
            c if c.is_ascii() => out.push(c),
            c => out.push_str(&format!("\\u{:04X}", c as u32)),
        }
    }

    out
}
