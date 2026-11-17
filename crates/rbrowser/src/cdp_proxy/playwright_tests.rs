//! RB-040-06 end to end: Playwright's `chromium.connectOverCDP` (the pinned playwright-core of
//! `sidecar/`) against the proxy in front of the scripted browser. The Node half is
//! `sidecar/e2e/connect-over-cdp.ts`; it reads the answer of `cdp_endpoint` from a file, as the
//! example in `examples/playwright/` does, and prints one JSON report.
//!
//! Ignored by default - it needs `node` and `sidecar/node_modules` (`npm ci`);
//! `scripts/check.sh --rust` runs it as the stage `playwright-e2e`, after the preflight installed
//! the sidecar's packages. The real CEF counterpart is the manual acceptance of the job file.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rb_cdp::ProxyToken;
use rb_process::NoConsoleWindow as _;
use serde_json::{Value, json};
use tokio::runtime::Handle;

use super::endpoint::{Facts, WslRoute, answer};
use super::server::{Hooks, ProxyServer};
use crate::cdp::fake::FakeBrowser;
use crate::cdp::lock;

const TOKEN: &str = "5e7a1c0de5e7a1c0de5e7a1c0de5e7a1c0de5e7a1c0de5e7a1c0de5e7a1c0de0";

/// The views and the app's own pages, as the scripted browser lists them.
const TARGETS: &[(&str, &str)] = &[
    ("SHELL", crate::shell::SHELL_URL),
    ("SETTINGS", crate::shell::PAGES_URL),
    ("DEVTOOLS", "devtools://devtools/bundled/inspector.html"),
    ("V1", "https://a.test/"),
    ("V2", "https://b.test/"),
];

/// The run of the Node half: its report and everything it printed.
struct Run {
    report: Value,
    printed: String,
}

async fn run_playwright(answer: &Value) -> Run {
    let sidecar = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../sidecar");
    assert!(
        sidecar.join("node_modules/playwright-core").is_dir(),
        "run `npm ci` in sidecar/ first (scripts/check.sh --preflight does)"
    );
    let dir = tempfile::tempdir().expect("a temporary directory");
    let file = dir.path().join("cdp-endpoint.json");
    std::fs::write(&file, answer.to_string()).expect("writes the answer");
    let mut node = tokio::process::Command::new("node");
    node.arg("e2e/connect-over-cdp.ts")
        .args(["https://a.test/", "https://b.test/", "https://b.test/next"])
        .current_dir(&sidecar)
        .env("RBROWSER_CDP_ENDPOINT", &file)
        .kill_on_drop(true)
        .no_console_window();
    let output = tokio::time::timeout(Duration::from_secs(120), node.output())
        .await
        .expect("Playwright finishes in time")
        .expect("node starts");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let report = stdout
        .lines()
        .last()
        .and_then(|line| serde_json::from_str(line).ok())
        .unwrap_or(Value::Null);
    assert!(
        output.status.success(),
        "the Node half failed: {report}\n{stdout}\n{stderr}"
    );
    Run {
        report,
        printed: format!("{stdout}{stderr}"),
    }
}

#[tokio::test]
#[ignore = "needs node and sidecar/node_modules; scripts/check.sh runs it as the stage playwright-e2e"]
async fn playwright_connects_with_the_token_sees_only_the_views_and_leaves_their_size_alone() {
    let (fake, router) = FakeBrowser::connect();
    fake.auto_attach();
    for (id, url) in TARGETS {
        fake.add_target(id, url);
    }
    let ended = Arc::new(Mutex::new(Vec::new()));
    let record = ended.clone();
    let hooks = Hooks {
        scale: Arc::new(|target: &str| (target == "V1").then_some(0.5)),
        on_end: Arc::new(move |targets| lock(&record).push(targets)),
        filter: crate::devtools::hidden::filter(),
    };
    let token = ProxyToken::from_text(TOKEN.to_owned());
    let server = ProxyServer::start(&Handle::current(), router, None, token.clone(), hooks)
        .expect("listens");
    let facts = Facts {
        port: server.port(),
        token: &token,
        browser_id: server.browser_id(),
    };
    let wsl = WslRoute::Unavailable("not in this test".to_owned());
    let Run { report, printed } = run_playwright(&answer(&facts, None, &wsl)).await;

    // No token, a wrong one in the header or in the path: refused, no browser session opened.
    assert_eq!(
        report["refused"],
        json!({"none": true, "wrongHeader": true, "wrongPath": true})
    );
    assert_eq!(fake.sent("Target.attachToBrowserTarget").len(), 3);
    // Header, path and socket form: one context with exactly the views - no shell, settings
    // window or DevTools, which the auto-attach announced and the proxy detached again.
    let views = json!({"contexts": 1, "pages": ["https://a.test/", "https://b.test/"]});
    for form in ["header", "path", "ws"] {
        assert_eq!(report[form], views, "{form}: {report}");
    }
    let detached: BTreeSet<String> = fake
        .sent("Target.detachFromTarget")
        .iter()
        .filter_map(|message| message["params"]["sessionId"].as_str())
        .filter_map(|session| fake.target_of(session))
        .collect();
    for hidden in ["SHELL", "SETTINGS", "DEVTOOLS"] {
        assert!(detached.contains(hidden), "{hidden}: {detached:?}");
    }
    // page.setViewportSize resolves, but the view's metrics are rBrowser's: nothing was sent;
    // neither the media features nor the browser-wide download behaviour Playwright sets when it
    // connects without `noDefaults`.
    assert_eq!(report["viewportSet"], json!(true));
    for owned in [
        "Emulation.setEmulatedMedia",
        "Emulation.setDeviceMetricsOverride",
        "Emulation.clearDeviceMetricsOverride",
        "Browser.setWindowBounds",
        "Browser.setDownloadBehavior",
    ] {
        assert!(fake.sent(owned).is_empty(), "{owned}");
    }
    // A click in CSS px lands scaled in its view (ADR 0001 addendum).
    let pressed = fake
        .sent("Input.dispatchMouseEvent")
        .into_iter()
        .find(|message| message["params"]["type"] == "mousePressed")
        .expect("a press");
    assert_eq!(
        fake.target_of(pressed["sessionId"].as_str().unwrap_or_default()),
        Some("V1".to_owned())
    );
    assert_eq!(
        (&pressed["params"]["x"], &pressed["params"]["y"]),
        (&json!(50.0), &json!(30.0))
    );
    // page.goto navigates the chosen view, and only it.
    let navigations = fake.sent("Page.navigate");
    assert_eq!(navigations.len(), 1, "{navigations:?}");
    assert_eq!(
        fake.target_of(navigations[0]["sessionId"].as_str().unwrap_or_default()),
        Some("V2".to_owned())
    );
    assert_eq!(
        navigations[0]["params"]["url"],
        json!("https://b.test/next")
    );
    assert_eq!(report["navigated"], json!("https://b.test/next"));
    assert_eq!(report["clickedUrl"], json!("https://a.test/"));
    // newCDPSession needs a second browser session, which the proxy refuses.
    let cdp_session = report["cdpSession"].as_str().unwrap_or_default();
    assert!(
        cdp_session.contains("this connection is a browser session already"),
        "{cdp_session}"
    );
    // Every connection that got in ended, and the app re-applies the emulation of its views.
    for _ in 0..400 {
        if lock(&ended).len() == 3 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let ended = lock(&ended).clone();
    assert_eq!(ended.len(), 3, "{ended:?}");
    for touched in &ended {
        assert_eq!(touched, &["V1".to_owned(), "V2".to_owned()]);
    }
    assert!(!printed.contains(TOKEN), "the token was printed");
}
