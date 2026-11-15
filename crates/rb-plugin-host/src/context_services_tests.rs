//! The host functions against a wired service (Charge 6, host services): who calls, what the
//! grants let through, where the project comes from.

use std::sync::Arc;
use std::time::Duration;

use rb_plugin_types::{Finding, Severity, codes};

use super::PluginContext;
use crate::manifest::Grants;
use crate::test_support::ScriptedServices;

fn grants(table: &str) -> Grants {
    Grants::parse(&toml::from_str(table).expect("toml")).expect("grants")
}

fn plugin(table: &str, services: &Arc<ScriptedServices>) -> PluginContext {
    let services: Arc<dyn crate::HostServices> = services.clone();
    PluginContext::new("sample", "1.0.0", grants(table), services)
        .with_timeout(Duration::from_millis(2_500))
}

fn finding(rule: &str, message: &str) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity: Severity::Warning,
        message: message.to_owned(),
        ..Finding::default()
    }
}

#[tokio::test]
async fn evaluate_names_the_calling_plugin_and_its_time_limit() {
    let services = Arc::new(ScriptedServices::default());
    let context = plugin(
        "evaluate = true\nsidecar = { methods = [\"lighthouse.run\"] }",
        &services,
    );
    assert_eq!(context.evaluate(1, "1 + 1").await.expect("ok"), "\"ok\"");
    context
        .sidecar_call("lighthouse.run", "{}")
        .await
        .expect("listed");
    let callers = services.callers.lock().expect("callers").clone();
    assert_eq!(
        callers,
        vec![
            ("sample".to_owned(), Duration::from_millis(2_500)),
            ("sample".to_owned(), Duration::from_millis(2_500)),
        ]
    );
}

#[tokio::test]
async fn a_sidecar_method_the_manifest_does_not_list_is_refused() {
    let services = Arc::new(ScriptedServices::default());
    let context = plugin("sidecar = { methods = [\"lighthouse.run\"] }", &services);
    let refused = context
        .sidecar_call("webkit.launch", "{}")
        .await
        .expect_err("not listed");
    assert_eq!(refused.code(), Some(codes::DENIED));
    assert_eq!(refused.param("method"), Some("webkit.launch"));
    assert!(services.callers.lock().expect("callers").is_empty());
}

/// A plugin's Lighthouse run loads its URL in a browser of its own and answers the report with
/// screenshots: only the sites the views show, unless a person lifted the limit (security review
/// RB-090-06, finding 4; RB-090-21).
#[tokio::test]
async fn a_lighthouse_run_is_held_to_the_sites_of_the_views() {
    let services = Arc::new(ScriptedServices::default());
    services
        .urls
        .lock()
        .expect("urls")
        .push((2, "https://Shop.example:443/cart?x=1".to_owned()));
    let context = plugin("sidecar = { methods = [\"lighthouse.start\"] }", &services);
    for allowed in [
        r#"{"url":"https://shop.example/cart?x=1","label":"desktop"}"#,
        r#"{"url":"https://shop.example/other/page","label":"desktop"}"#,
    ] {
        context
            .sidecar_call("lighthouse.start", allowed)
            .await
            .expect(allowed);
    }
    for refused in [
        r#"{"url":"http://127.0.0.1:8080/admin","label":"desktop"}"#,
        r#"{"url":"http://192.168.1.1/","label":"desktop"}"#,
        r#"{"url":"http://shop.example/cart","label":"desktop"}"#,
        r#"{"url":"https://shop.example.evil/","label":"desktop"}"#,
        r#"{"url":"file:///C:/secret.html","label":"desktop"}"#,
        // A browser ends the host at the backslash (127.0.0.1); a naive split at `@` would not.
        r#"{"url":"https://127.0.0.1\\@shop.example/","label":"desktop"}"#,
        r#"{"url":"https://user@shop.example/","label":"desktop"}"#,
        r#"{"url":"https://shop.example\t.evil/","label":"desktop"}"#,
        r#"{"label":"desktop"}"#,
        "not json",
    ] {
        let failure = context
            .sidecar_call("lighthouse.start", refused)
            .await
            .expect_err(refused);
        assert_eq!(failure.code(), Some(codes::SIDECAR_URL_DENIED), "{refused}");
    }
    assert_eq!(services.callers.lock().expect("callers").len(), 2);
}

#[tokio::test]
async fn a_person_can_let_lighthouse_runs_reach_any_address() {
    let services = Arc::new(ScriptedServices::default());
    services
        .lighthouse_any_url
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let context = plugin("sidecar = { methods = [\"lighthouse.start\"] }", &services);
    context
        .sidecar_call("lighthouse.start", r#"{"url":"http://127.0.0.1:8080/"}"#)
        .await
        .expect("allowed by the person");
}

#[tokio::test]
async fn other_lighthouse_methods_carry_no_url_and_pass() {
    let services = Arc::new(ScriptedServices::default());
    let context = plugin(
        "sidecar = { methods = [\"lighthouse.poll\", \"lighthouse.status\"] }",
        &services,
    );
    for method in ["lighthouse.poll", "lighthouse.status"] {
        context
            .sidecar_call(method, r#"{"job":"1"}"#)
            .await
            .expect(method);
    }
}

#[tokio::test]
async fn cdp_methods_that_reach_the_hosts_own_instrumentation_are_refused() {
    let services = Arc::new(ScriptedServices::default());
    let context = plugin(
        "cdp = { domains = [\"Page\", \"Runtime\", \"DOM\"] }",
        &services,
    );
    for method in [
        "Page.close",
        "Page.crash",
        "Page.disable",
        "Page.removeScriptToEvaluateOnNewDocument",
        "Runtime.disable",
        "Runtime.addBinding",
        "Runtime.removeBinding",
        "Runtime.terminateExecution",
    ] {
        let refused = context.cdp_send(1, method, "{}").await.expect_err(method);
        assert_eq!(refused.code(), Some(codes::CDP_DOMAIN_DENIED), "{method}");
        assert_eq!(refused.param("method"), Some(method), "{method}");
        assert_eq!(refused.param("reason"), Some("method"), "{method}");
    }
    // `Fetch` is no domain a manifest may list at all.
    assert!(
        Grants::parse(&toml::from_str("cdp = { domains = [\"Fetch\"] }").expect("toml")).is_err()
    );
    assert_eq!(
        context
            .cdp_send(1, "DOM.getDocument", "{}")
            .await
            .expect("listed"),
        "{}"
    );
}

#[tokio::test]
async fn the_project_comes_from_the_services_when_the_load_had_none() {
    let project = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(project.path().join(".rbrowser")).expect("dir");
    std::fs::write(project.path().join(".rbrowser/config.json"), b"{}").expect("write");
    let services = Arc::new(ScriptedServices::default());
    *services.project.lock().expect("project") = Some(project.path().to_path_buf());
    let context = plugin("project-read = true\nout-write = true", &services);
    assert_eq!(
        context
            .project_read(".rbrowser/config.json")
            .await
            .expect("read"),
        b"{}"
    );
    context.out_write("state.json", b"[]").await.expect("write");
    assert!(
        project
            .path()
            .join(".rbrowser/out/sample/state.json")
            .is_file()
    );
}

#[tokio::test]
async fn settings_come_from_the_services_first() {
    let services = Arc::new(ScriptedServices::default());
    services.settings.lock().expect("settings").push((
        "sample".to_owned(),
        "maxPages".to_owned(),
        "25".to_owned(),
    ));
    let context = plugin("views = true", &services).with_settings(vec![
        ("maxPages".to_owned(), "10".to_owned()),
        ("other".to_owned(), "1".to_owned()),
    ]);
    assert_eq!(context.setting("maxPages").as_deref(), Some("25"));
    assert_eq!(context.setting("other").as_deref(), Some("1"));
    assert_eq!(context.setting("missing"), None);
}

#[tokio::test]
async fn findings_are_held_to_the_plugins_own_message_keys() {
    let services = Arc::new(ScriptedServices::default());
    let context = plugin("findings = true", &services);
    context
        .report_findings(vec![finding("title.missing", "plugin.sample.title")])
        .expect("own key");
    let refused = context
        .report_findings(vec![finding("title.missing", "plugin.other.title")])
        .expect_err("another plugin's key");
    assert_eq!(refused.code(), Some(codes::INVALID));
    let refused = context
        .report_findings(vec![finding("bad rule!", "plugin.sample.title")])
        .expect_err("malformed rule");
    assert_eq!(refused.code(), Some(codes::INVALID));
    assert_eq!(services.findings.lock().expect("findings").len(), 1);
}

/// A project with a snippet checked in, served by `services`.
fn project_with_snippet(services: &ScriptedServices) -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(project.path().join(".rbrowser/snippets")).expect("dir");
    std::fs::write(project.path().join(".rbrowser/snippets/a.js"), b"1").expect("write");
    std::fs::write(project.path().join(".rbrowser/config.json"), b"{}").expect("write");
    *services.project.lock().expect("project") = Some(project.path().to_path_buf());
    project
}

#[tokio::test]
async fn the_trust_protected_folders_are_refused_in_an_untrusted_workspace() {
    let services = Arc::new(ScriptedServices::default());
    let _project = project_with_snippet(&services);
    let context = plugin("project-read = true", &services);
    for path in [
        ".rbrowser/snippets/a.js",
        ".rbrowser/flows/x.ts",
        ".rbrowser/mocks/m.json",
    ] {
        let failure = context.project_read(path).await.expect_err("untrusted");
        assert_eq!(failure.code(), Some(codes::UNTRUSTED_WORKSPACE), "{path}");
    }
    let failure = context
        .project_list(".rbrowser/snippets")
        .await
        .expect_err("untrusted");
    assert_eq!(failure.code(), Some(codes::UNTRUSTED_WORKSPACE));
    // The rest of `.rbrowser/` stays readable, and the folder's name may be listed.
    assert_eq!(
        context
            .project_read(".rbrowser/config.json")
            .await
            .expect("read"),
        b"{}"
    );
    let names = context.project_list(".rbrowser").await.expect("listed");
    assert!(names.contains(&"snippets/".to_owned()), "{names:?}");
}

#[tokio::test]
async fn the_trust_protected_folders_are_read_once_the_workspace_is_trusted() {
    let services = Arc::new(ScriptedServices::default());
    let _project = project_with_snippet(&services);
    services
        .trusted
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let context = plugin("project-read = true", &services);
    assert_eq!(
        context
            .project_read(".rbrowser/snippets/a.js")
            .await
            .expect("read"),
        b"1"
    );
    assert_eq!(
        context
            .project_list(".rbrowser/snippets")
            .await
            .expect("listed"),
        vec!["a.js".to_owned()]
    );
}
