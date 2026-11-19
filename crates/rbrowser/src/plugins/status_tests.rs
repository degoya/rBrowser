use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rb_plugin_types::Failure;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::cdp::lock;
use crate::plugins::runtime::{Component, Plugin, PluginRuntime};

const LOCALE: &[u8] = br#"{"plugin":{"status-test":{"name":"Status test","notice":{
    "picking":"Click a pixel in a view, Esc to stop.",
    "picked":"Picked {value}."}}}}"#;

fn register() {
    rb_i18n::register_plugin_locale("status-test", "en", LOCALE).expect("registers");
}

#[test]
fn a_notice_shows_the_plugins_own_text_with_its_parameters() {
    register();
    let notice = notification_of(
        "status-test",
        &json!({ "key": "plugin.status-test.notice.picked", "params": { "value": "#1e88e5" },
                 "level": "success" }),
    )
    .expect("a notice");
    assert_eq!(notice.level, NotificationLevel::Success);
    assert_eq!(notice.message_key, PLUGIN_STATUS_KEY);
    // Under the plugin's name (review S1): `{plugin}: {text}`.
    assert_eq!(
        notice.args.get("plugin").map(String::as_str),
        Some("Status test")
    );
    assert_eq!(
        notice.args.get("text").map(String::as_str),
        Some("Picked #1e88e5.")
    );
    let plain = notification_of(
        "status-test",
        &json!({ "key": "plugin.status-test.notice.picking" }),
    )
    .expect("a notice");
    assert_eq!(plain.level, NotificationLevel::Info);
}

#[test]
fn a_notice_that_does_not_hold_shows_nothing() {
    register();
    let long = "x".repeat(MAX_PARAM_CHARS + 1);
    let many: serde_json::Map<String, Value> = (0..=MAX_PARAMS)
        .map(|index| (format!("p{index}"), json!("v")))
        .collect();
    for message in [
        // Another plugin's text, the app's own, an unknown key.
        json!({ "key": "plugin.other.notice.picked" }),
        json!({ "key": "plugin.status-testing.notice.picked" }),
        json!({ "key": "plugins.notice.command_done" }),
        json!({ "key": "plugin.status-test.notice.unknown" }),
        // A warning or an error the plugin could dress up as the app's.
        json!({ "key": "plugin.status-test.notice.picking", "level": "error" }),
        json!({ "key": "plugin.status-test.notice.picking", "level": "warning" }),
        json!({ "key": "plugin.status-test.notice.picked", "params": { "value": long } }),
        json!({ "key": "plugin.status-test.notice.picked", "params": { "value": [1] } }),
        json!({ "key": "plugin.status-test.notice.picked", "params": many }),
        json!({ "key": "plugin.status-test.notice.picked", "params": "value" }),
        json!({ "key": 7 }),
        json!("plugin.status-test.notice.picking"),
    ] {
        assert!(
            notification_of("status-test", &message).is_none(),
            "{message}"
        );
    }
}

#[test]
fn control_and_format_characters_are_taken_out_and_the_text_cut() {
    register();
    let sneaky = "#fff\n\u{202e}gnp.exe\u{200b}\u{0007}";
    let notice = notification_of(
        "status-test",
        &json!({ "key": "plugin.status-test.notice.picked", "params": { "value": sneaky } }),
    )
    .expect("a notice");
    assert_eq!(
        notice.args.get("text").map(String::as_str),
        Some("Picked #fffgnp.exe.")
    );
    assert_eq!(clean(&"a".repeat(10)), "a".repeat(10));
    let long = format!("{}\u{2066}", "x".repeat(MAX_PARAM_CHARS));
    let notice = notification_of(
        "status-test",
        &json!({ "key": "plugin.status-test.notice.picked", "params": { "value": long, "other": "y" } }),
    )
    .expect("a notice");
    assert!(notice.args["text"].chars().count() <= MAX_TEXT_CHARS);
}

#[test]
fn one_notice_a_second_per_plugin_the_newest_waits() {
    let throttle = Throttle::default();
    let start = std::time::Instant::now();
    let notice = |text: &str| Notification {
        level: NotificationLevel::Info,
        message_key: PLUGIN_STATUS_KEY.to_owned(),
        args: BTreeMap::from([("text".to_owned(), text.to_owned())]),
        settings_key: None,
    };
    assert_eq!(
        throttle.offer("a", notice("1"), start),
        Offered::Now(notice("1"))
    );
    let soon = start + Duration::from_millis(300);
    assert_eq!(
        throttle.offer("a", notice("2"), soon),
        Offered::Later(Duration::from_millis(700))
    );
    assert_eq!(throttle.offer("a", notice("3"), soon), Offered::Replaced);
    // Another plugin is not held back.
    assert_eq!(
        throttle.offer("b", notice("x"), soon),
        Offered::Now(notice("x"))
    );
    let later = start + Duration::from_secs(1);
    assert_eq!(throttle.take("a", later), Some(notice("3")));
    assert_eq!(throttle.take("a", later), None);
    assert_eq!(
        throttle.offer("a", notice("4"), later + Duration::from_secs(1)),
        Offered::Now(notice("4"))
    );
}

/// A component answering every call with `answer`.
struct Fixed {
    answer: String,
}

#[async_trait]
impl Component for Fixed {
    async fn page_message(&self, _view: u32, _message: &Value) -> Result<String, Failure> {
        Ok(self.answer.clone())
    }

    async fn run_command(&self, _command: &str, _args: &str) -> Result<String, Failure> {
        Ok(self.answer.clone())
    }

    async fn call_tool(&self, _tool: &str, _args: &str) -> Result<String, Failure> {
        Ok(self.answer.clone())
    }
}

type Seen = Arc<Mutex<Vec<(String, String)>>>;

/// A runtime serving plugin `status-test` from `source`, granted `status` or not; what reached
/// the status bar (plugin, text) and the host (plugin, action).
fn serve(answer: &Value, source: PluginSource, status: bool) -> (Seen, Seen, Arc<PluginRuntime>) {
    serve_as("status-test", answer, source, status)
}

/// [`serve`] for the plugin `id`.
fn serve_as(
    id: &str,
    answer: &Value,
    source: PluginSource,
    status: bool,
) -> (Seen, Seen, Arc<PluginRuntime>) {
    register();
    let (_fake, router) = FakeBrowser::connect();
    let (notices, actions): (Seen, Seen) = (Arc::default(), Arc::default());
    let into = notices.clone();
    let notice: NoticeSink = Arc::new(move |plugin, notice| {
        let text = notice.args.get("text").cloned().unwrap_or_default();
        lock(&into).push((plugin.to_owned(), text));
    });
    let into = actions.clone();
    let host: HostSink = Arc::new(move |plugin, message| {
        let action = message["action"].as_str().unwrap_or_default().to_owned();
        lock(&into).push((plugin.to_owned(), action));
    });
    let plugin = Plugin {
        id: id.to_owned(),
        source,
        component: Arc::new(Fixed {
            answer: answer.to_string(),
        }),
        bridge: None,
        console: false,
        network: false,
        status,
    };
    let sinks = Sinks {
        panels: Arc::new(|_, _| {}),
        status: notice,
        host,
    };
    let runtime = PluginRuntime::new(vec![plugin], router, Handle::current(), sinks);
    (notices, actions, runtime)
}

fn posting(target: &str, message: Value) -> Value {
    json!({ "$post": [{ "target": target, "message": message }] })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_status_post_reaches_the_status_bar_and_a_command_knows_it_showed_one() {
    let answer = posting(
        "status",
        json!({ "key": "plugin.status-test.notice.picking" }),
    );
    let (notices, _, runtime) = serve(&answer, PluginSource::Bundled, true);
    let (value, noted) = runtime
        .run_command_noted("status-test", "pick", &json!({}))
        .await
        .expect("answers");
    assert_eq!(value, json!({}));
    assert!(noted, "the command's generic done is not shown");
    // A page request's notice too: the pipette ended by Esc in a view.
    use crate::plugins::pages::Inbox as _;
    runtime
        .request(
            "status-test",
            rb_engine::ViewId(2),
            json!({ "type": "stopped" }),
        )
        .await
        .expect("answers");
    let text = "Click a pixel in a view, Esc to stop.".to_owned();
    assert_eq!(
        lock(&notices).clone(),
        vec![
            ("status-test".to_owned(), text.clone()),
            ("status-test".to_owned(), text)
        ]
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plugin_without_the_status_grant_shows_nothing_and_keeps_its_done() {
    // Review S1 and C5: refused, and a refused notice is no notice.
    let answer = posting(
        "status",
        json!({ "key": "plugin.status-test.notice.picking" }),
    );
    let (notices, _, runtime) = serve(&answer, PluginSource::Bundled, false);
    let (_, noted) = runtime
        .run_command_noted("status-test", "pick", &json!({}))
        .await
        .expect("answers");
    assert!(!noted);
    assert!(lock(&notices).is_empty());
    // A granted plugin whose notice does not hold keeps its done as well.
    let answer = posting(
        "status",
        json!({ "key": "plugin.status-test.notice.unknown" }),
    );
    let (_, _, runtime) = serve(&answer, PluginSource::Bundled, true);
    let (_, noted) = runtime
        .run_command_noted("status-test", "pick", &json!({}))
        .await
        .expect("answers");
    assert!(!noted);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_host_post_runs_only_from_a_bundled_plugins_command() {
    // Review S3: not from a tool or a page request (no gesture), not from a plugin of the same
    // id a user or a project installed.
    let answer = posting("host", json!({ "action": "add_design" }));
    let (_, actions, runtime) = serve(&answer, PluginSource::Bundled, true);
    runtime
        .call_tool("status-test", "designs", &json!({}))
        .await
        .expect("answers");
    use crate::plugins::pages::Inbox as _;
    runtime
        .request("status-test", rb_engine::ViewId(2), json!({}))
        .await
        .expect("answers");
    assert!(lock(&actions).is_empty());
    runtime
        .run_command("status-test", "add_design", &json!({}))
        .await
        .expect("answers");
    assert_eq!(
        lock(&actions).clone(),
        vec![("status-test".to_owned(), "add_design".to_owned())]
    );
    for source in [PluginSource::User, PluginSource::Project] {
        let (_, actions, runtime) = serve(&answer, source, true);
        runtime.set_enabled(&std::collections::BTreeSet::new(), true);
        runtime
            .run_command("status-test", "add_design", &json!({}))
            .await
            .expect("answers");
        assert!(lock(&actions).is_empty(), "{source:?}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn the_form_autofills_fill_report_runs_from_its_tool_too_but_never_from_a_page_request() {
    // RB-090-78: a fill from the panel or MCP is recorded as one from the command; recording
    // asks no one and opens nothing. A page request still runs no host post.
    let answer = posting(
        "host",
        json!({ "action": "record_fill", "view": 1, "fields": [] }),
    );
    let (_, actions, runtime) = serve_as("form-autofill", &answer, PluginSource::Bundled, false);
    use crate::plugins::pages::Inbox as _;
    runtime
        .request("form-autofill", rb_engine::ViewId(2), json!({}))
        .await
        .expect("answers");
    assert!(lock(&actions).is_empty());
    runtime
        .call_tool("form-autofill", "fill", &json!({}))
        .await
        .expect("answers");
    runtime
        .run_command("form-autofill", "fill_valid", &json!({}))
        .await
        .expect("answers");
    let seen = ("form-autofill".to_owned(), "record_fill".to_owned());
    assert_eq!(lock(&actions).clone(), vec![seen.clone(), seen]);
    // Not from a plugin of the same id a user installed.
    let (_, actions, runtime) = serve_as("form-autofill", &answer, PluginSource::User, false);
    runtime.set_enabled(&std::collections::BTreeSet::new(), true);
    runtime
        .call_tool("form-autofill", "fill", &json!({}))
        .await
        .expect("answers");
    assert!(lock(&actions).is_empty());
}

/// The bundled plugins' folder: `plugins/` of the checkout.
fn plugins_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

#[test]
fn the_colour_pickers_notice_names_the_plugin_once() {
    // RB-080-76: the core frames a notice as `{plugin}: {text}`; the colour picker's own texts
    // began with "Colour picker:" as well - "Colour picker: Colour picker: picked #1e88e5.".
    let locale = std::fs::read(plugins_dir().join("color-picker/locales/en.json")).expect("locale");
    rb_i18n::register_plugin_locale("color-picker", "en", &locale).expect("registers");
    for (name, params) in [
        ("picking", json!({})),
        ("picked", json!({ "value": "#1e88e5" })),
        ("stopped", json!({})),
    ] {
        let notice = notification_of(
            "color-picker",
            &json!({ "key": format!("plugin.color-picker.notice.{name}"), "params": params }),
        )
        .expect("a notice");
        let plugin = notice
            .args
            .get("plugin")
            .map(String::as_str)
            .unwrap_or_default();
        let text = notice
            .args
            .get("text")
            .map(String::as_str)
            .unwrap_or_default();
        assert_eq!(plugin, "Colour picker");
        assert!(
            !text.to_lowercase().contains("colour picker"),
            "{name}: {plugin}: {text}"
        );
    }
}

#[test]
fn no_status_text_of_a_bundled_plugin_names_the_plugin() {
    // RB-080-76, docs/plugins.md ("Posts"): the core names the plugin; every plugin granted
    // `status` keeps its own texts without it.
    let mut checked = 0;
    for entry in std::fs::read_dir(plugins_dir()).expect("plugins") {
        let dir = entry.expect("entry").path();
        let manifest = std::fs::read_to_string(dir.join("manifest.toml")).unwrap_or_default();
        if !manifest.lines().any(|line| line.trim() == "status = true") {
            continue;
        }
        let id = dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("id")
            .to_owned();
        let locale: Value =
            serde_json::from_slice(&std::fs::read(dir.join("locales/en.json")).expect("locale"))
                .expect("json");
        let own = &locale["plugin"][&id];
        // The locale's name, else the manifest's (the status bar shows one of them).
        let manifest_name = manifest
            .lines()
            .find_map(|line| line.strip_prefix("name = "))
            .map(|name| name.trim_matches('"'))
            .expect("the manifest's name");
        let name = own["name"].as_str().unwrap_or(manifest_name).to_lowercase();
        for group in ["notice", "status"] {
            let Some(texts) = own[group].as_object() else {
                continue;
            };
            for (key, text) in texts {
                let text = text.as_str().unwrap_or_default().to_lowercase();
                assert!(
                    !text.starts_with(&name),
                    "{id}: {group}.{key} names the plugin: {text}"
                );
                checked += 1;
            }
        }
    }
    // The colour picker's and the image overlay's.
    assert!(checked >= 4, "{checked}");
}
