use rb_plugin_types::FailureKind;

use super::*;

#[test]
fn the_panels_window_opens_at_a_well_formed_panel_only() {
    assert_eq!(
        panels_url(&json!({ "plugin": "hello-panel", "panel": "main" })),
        "rbrowser://ui/settings.html#/panels/hello-panel.main"
    );
    for args in [
        Value::Null,
        json!({ "plugin": "hello-panel" }),
        json!({ "plugin": "Hello", "panel": "main" }),
        json!({ "plugin": "hello-panel", "panel": "main/../../x" }),
        json!({ "plugin": "hello-panel", "panel": "a#b" }),
    ] {
        assert_eq!(
            panels_url(&args),
            "rbrowser://ui/settings.html#/panels",
            "{args}"
        );
    }
}

#[test]
fn a_plugin_failure_keeps_its_code_and_parameters() {
    let failure =
        Failure::coded(FailureKind::Other, "plugin.hello-panel.error.nope").with_param("n", 2);
    assert_eq!(failure_text(&failure), "plugin.hello-panel.error.nope n=2");
    // No text anywhere: the code and its parameters stand in.
    assert_eq!(
        command_error("hello-panel", failure).to_string(),
        "command failed: plugin.hello-panel.error.nope n=2"
    );
}

/// RB-080-58: the status bar showed `plugin.lighthouse.errors.no_browser` - the plugin's error
/// reads as the sentence of its own catalogue, its parameters filled in.
#[test]
fn a_plugin_failure_reads_as_the_sentence_of_its_catalogue() {
    let locale = br#"{"plugin":{"msg-text":{"errors":{"no_browser":"No browser to run in ({reason}). Install Chrome."}}}}"#;
    rb_i18n::register_plugin_locale("msg-text", "en", locale).expect("registers");
    let failure = Failure::coded(
        FailureKind::NotSupported,
        "plugin.msg-text.errors.no_browser",
    )
    .with_param("reason", "none found");
    assert_eq!(
        failure_message("msg-text", &failure),
        "No browser to run in (none found). Install Chrome."
    );
    assert_eq!(
        command_error("msg-text", failure).to_string(),
        "command failed: No browser to run in (none found). Install Chrome."
    );
}

/// RB-080-58 review 3: a plugin cannot show another plugin's text, nor a core text that is no
/// plugin failure - it would speak with another's voice across a trust boundary.
#[test]
fn a_plugin_cannot_borrow_another_plugins_or_the_cores_text() {
    let locale = br#"{"plugin":{"spoof-victim":{"errors":{"trust":"Your workspace is trusted; click OK."}}}}"#;
    rb_i18n::register_plugin_locale("spoof-victim", "en", locale).expect("registers");
    let foreign = Failure::coded(FailureKind::Other, "plugin.spoof-victim.errors.trust");
    assert_eq!(
        failure_message("spoof-attacker", &foreign),
        "plugin.spoof-victim.errors.trust"
    );
    assert_eq!(
        failure_message("spoof-victim", &foreign),
        "Your workspace is trusted; click OK."
    );
    // A core text that exists but is no plugin failure code.
    let core = Failure::coded(FailureKind::Other, "commands.notice.failed")
        .with_param("command", "x")
        .with_param("error", "y");
    assert_eq!(
        failure_message("spoof-attacker", &core),
        "commands.notice.failed command=x error=y"
    );
}

/// A host code (`plugins.error.*`) reads as the core catalogue's sentence.
#[test]
fn a_host_failure_reads_as_the_core_sentence() {
    let failure = Failure::coded(FailureKind::NotFound, rb_plugin_types::codes::NOT_FOUND);
    let message = failure_message("any-plugin", &failure);
    assert_ne!(message, rb_plugin_types::codes::NOT_FOUND);
    assert!(!message.starts_with("plugins."), "{message}");
}

/// RB-080-58 review 4: a code without a text is logged once, not on every list build.
#[test]
fn a_code_without_a_text_is_reported_once() {
    assert!(first_missing("plugin.once-only.errors.nope"));
    assert!(!first_missing("plugin.once-only.errors.nope"));
    assert!(first_missing("plugin.once-only.errors.other"));
}

fn core() -> (Arc<crate::host::ui::recording::RecordingUi>, Arc<Core>) {
    use crate::cdp::fake::FakeBrowser;
    use crate::control::CoreSetup;
    use crate::host::Host;
    use crate::host::state::default_config;
    use crate::host::ui::recording::RecordingUi;
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, tokio::runtime::Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(
        host,
        ui.clone(),
        tokio::runtime::Handle::current(),
        CoreSetup::default(),
    );
    (ui, core)
}

/// A plugin command that opened no panel has no effect the host can see (`plugin.seo.check`
/// reports its findings to a panel that may be closed), so the host names its end in the status
/// bar, and a check's findings window is opened (RB-080-18).
#[tokio::test]
async fn a_plugin_command_without_a_panel_reports_its_end_and_a_check_opens_the_findings() {
    use crate::host::ui::recording::UiCall;
    let (ui, core) = core();
    crate::panel_dock::tests::float(&core);
    report_done(&core, "seo", "plugin.seo.commands.check", Group::Checks);
    let items = core.status_log().items();
    let last = items.last().expect("a status message");
    assert_eq!(last.message_key, COMMAND_DONE_KEY);
    assert_eq!(last.level, rb_ipc::NotificationLevel::Info);
    // No locale registered in this test: the title key stands in for the text.
    assert_eq!(
        last.args.get("command").map(String::as_str),
        Some("plugin.seo.commands.check")
    );
    assert!(
        ui.calls()
            .iter()
            .any(|call| matches!(call, UiCall::PageWindow(crate::page_window::PageWindow::Panels, url) if url.ends_with("#/panels/findings"))),
        "a check's findings open in the panels page"
    );
    let before = ui.calls().len();
    report_done(
        &core,
        "overlays",
        "plugin.overlays.commands.outlines",
        Group::Overlays,
    );
    assert_eq!(
        core.status_log().items().last().expect("logged").args["command"],
        "plugin.overlays.commands.outlines"
    );
    assert!(
        !ui.calls()[before..]
            .iter()
            .any(|call| matches!(call, UiCall::PageWindow(..))),
        "only a check opens the findings window"
    );
}
