use std::time::{Duration, Instant};

use rb_engine::ViewId;
use rb_plugin_host::LogLevel;

use super::*;

fn values(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn a_host_code_reads_as_its_sentence_and_the_own_prefix_goes() {
    // The owner's console: "[rulers-guides] plugins.error.busy".
    assert_eq!(
        sentence(
            "rulers-guides",
            &values(&["[rulers-guides]", "plugins.error.busy"])
        ),
        "The plugin is still answering earlier requests."
    );
}

#[test]
fn text_and_unknown_codes_stay_as_they_are() {
    assert_eq!(
        sentence("demo", &values(&["drawn", "3", "views"])),
        "drawn 3 views"
    );
    assert_eq!(
        sentence("demo", &values(&["plugins.error.no_such_code"])),
        "plugins.error.no_such_code"
    );
    // Another plugin's prefix and code are no one's to read.
    assert_eq!(
        sentence("demo", &values(&["[other]", "plugin.other.errors.x"])),
        "[other] plugin.other.errors.x"
    );
}

#[test]
fn the_plugins_own_code_reads_from_its_catalogue() {
    rb_i18n::register_plugin_locale(
        "page-log-demo",
        "en",
        br#"{"plugin":{"page-log-demo":{"errors":{"nope":"Nothing to draw."}}}}"#,
    )
    .expect("a catalogue");
    assert_eq!(
        sentence(
            "page-log-demo",
            &values(&["plugin.page-log-demo.errors.nope"])
        ),
        "Nothing to draw."
    );
    rb_i18n::unregister_plugin_locales("page-log-demo");
}

#[test]
fn the_log_takes_a_burst_per_minute_then_notes_once() {
    let start = Instant::now();
    let mut gate = LogGate::new(start);
    for _ in 0..LOG_BURST {
        assert_eq!(gate.admit(start), Admit::Write);
    }
    assert_eq!(gate.admit(start), Admit::FirstHeld);
    assert_eq!(gate.admit(start), Admit::Held);
    assert_eq!(
        gate.admit(start + LOG_WINDOW + Duration::from_secs(1)),
        Admit::Write
    );
}

#[test]
fn the_feed_keeps_a_views_lines_with_their_source() {
    let mut lines = PluginLines::default();
    lines.push(
        ViewId(2),
        "rulers-guides",
        LogLevel::Warning,
        "busy",
        None,
        5.0,
    );
    lines.push(
        ViewId(3),
        "rulers-guides",
        LogLevel::Error,
        "other view",
        None,
        6.0,
    );
    let of_two = lines.of_view(ViewId(2));
    assert_eq!(of_two.len(), 1);
    assert_eq!(of_two[0]["source"], SOURCE);
    assert_eq!(of_two[0]["plugin"], "rulers-guides");
    assert_eq!(of_two[0]["level"], "warning");
    assert_eq!(of_two[0]["url"], "rbrowser://plugin/rulers-guides");
    assert_eq!(of_two[0]["time"], 5.0);
    assert!(of_two[0].get("stack").is_none());
}

// Review of RB-080-66, 8: a chatty plugin evicts only its own lines; a closed view's lines go.
#[test]
fn each_plugin_keeps_its_own_share_and_a_closed_views_lines_go() {
    let mut lines = PluginLines::default();
    lines.push(ViewId(2), "quiet", LogLevel::Error, "once", None, 1.0);
    let mut time = 1.0;
    for n in 0..(MAX_LINES_PER_PLUGIN * 3) {
        time += 1.0;
        lines.push(
            ViewId(2),
            "chatty",
            LogLevel::Log,
            &n.to_string(),
            None,
            time,
        );
    }
    let of_two = lines.of_view(ViewId(2));
    assert_eq!(of_two.len(), MAX_LINES_PER_PLUGIN + 1);
    assert_eq!(
        of_two[0]["plugin"], "quiet",
        "oldest first, the quiet plugin's line kept"
    );
    lines.forget_view(ViewId(2));
    assert!(lines.of_view(ViewId(2)).is_empty());
}

// Review of RB-080-66, 4: a page script's throw keeps its stack in the feed.
#[test]
fn a_failure_carries_its_stack() {
    let mut lines = PluginLines::default();
    lines.push(
        ViewId(1),
        "demo",
        LogLevel::Error,
        "boom",
        Some("Error: boom\n  at run"),
        1.0,
    );
    assert_eq!(
        lines.of_view(ViewId(1))[0]["stack"],
        "Error: boom\n  at run"
    );
}

#[test]
fn a_line_keeps_to_one_line_and_its_credentials_are_masked() {
    // A page script's text is untrusted: no forged log lines, no token in the log or the feed.
    let text = sentence(
        "demo",
        &values(&[
            "first\nERROR forged\r\tline",
            "https://x.test/a?token=s3cr3tvalue",
        ]),
    );
    assert!(!text.contains('\n') && !text.contains('\r') && !text.contains('\t'));
    assert!(!text.contains("s3cr3tvalue"), "{text}");
}
