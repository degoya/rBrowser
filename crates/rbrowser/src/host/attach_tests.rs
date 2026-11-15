use std::path::PathBuf;
use std::time::Duration;

use rb_config::{ColorScheme, EmulationFields, ProjectConfig};
use rb_engine::{CellRect, ViewHost};
use serde_json::json;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::ui::recording::{RecordingUi, UiCall};

struct Setup {
    fake: Arc<FakeBrowser>,
    ui: Arc<RecordingUi>,
    host: Arc<Host>,
}

/// One mobile breakpoint, dark and German, with a header from the project, one from the
/// project's defaults and one the merge took from `config.local.json`.
fn config() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "startUrl": "about:blank",
        "emulation": { "headers": { "X-Team": "web", "X-Local": "mine" } },
        "breakpoints": [{
            "id": "mobile", "width": 393, "height": 852, "touch": true,
            "colorScheme": "dark", "locale": "de-DE",
            "headers": { "X-Project": "checked-in" }
        }]
    }))
    .expect("a config")
}

/// A project folder whose `config.local.json` sets `X-Local`.
fn project_with_local_file(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("rb-010-05-{name}-{}", std::process::id()));
    let dir = root.join(".rbrowser");
    std::fs::create_dir_all(&dir).expect("the folder");
    std::fs::write(
        dir.join("config.local.json"),
        r#"{ "emulation": { "headers": { "X-Local": "mine" } } }"#,
    )
    .expect("the local file");
    root
}

fn setup(config: ProjectConfig, root: Option<PathBuf>) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(config, root).expect("opens");
    fake.add_target("T1", &marker_url(ViewId(1)));
    Setup { fake, ui, host }
}

async fn until(mut done: impl FnMut() -> bool) {
    for _ in 0..200 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("condition not reached");
}

async fn attach(setup: &Setup) {
    setup.host.on_view_bounds(
        ViewId(1),
        CellRect {
            x: 0,
            y: 44,
            width: 400,
            height: 800,
        },
    );
    setup.host.on_load_end(ViewId(1), &marker_url(ViewId(1)));
    until(|| !setup.fake.sent("Emulation.setLocaleOverride").is_empty()).await;
}

fn header_calls(setup: &Setup) -> Vec<Value> {
    setup
        .fake
        .sent("Network.setExtraHTTPHeaders")
        .into_iter()
        .map(|message| message["params"]["headers"].clone())
        .collect()
}

#[tokio::test]
async fn attaching_applies_the_whole_emulation_on_the_session() {
    let setup = setup(config(), None);
    attach(&setup).await;
    let media = setup.fake.sent("Emulation.setEmulatedMedia");
    assert_eq!(media[0]["sessionId"], "S1");
    assert_eq!(
        media[0]["params"]["features"],
        json!([{ "name": "prefers-color-scheme", "value": "dark" }])
    );
    assert_eq!(
        setup.fake.sent("Emulation.setTouchEmulationEnabled")[0]["params"],
        json!({ "enabled": true, "maxTouchPoints": 5 })
    );
    assert_eq!(
        setup.fake.sent("Emulation.setUserAgentOverride")[0]["params"]["acceptLanguage"],
        "de-DE,de;q=0.9"
    );
    let metrics = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    assert_eq!(metrics[0]["params"]["mobile"], true);
}

#[tokio::test]
async fn an_untrusted_project_sends_no_header_of_either_file_until_trusted() {
    let root = project_with_local_file("untrusted");
    let setup = setup(config(), Some(root.clone()));
    attach(&setup).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(
        header_calls(&setup).is_empty(),
        "a committed config.local.json sends nothing without trust"
    );
    setup.host.update_shell_state(|shell| shell.trusted = true);
    setup.host.reapply_emulation(ViewId(1));
    until(|| !header_calls(&setup).is_empty()).await;
    assert_eq!(
        header_calls(&setup)[0],
        json!({ "X-Local": "mine", "X-Project": "checked-in", "X-Team": "web" })
    );
    setup.host.update_shell_state(|shell| shell.trusted = false);
    setup.host.reapply_emulation(ViewId(1));
    until(|| header_calls(&setup).len() == 2).await;
    assert_eq!(header_calls(&setup)[1], json!({}));
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn without_a_local_file_an_untrusted_project_sends_no_header_and_no_network_domain() {
    let setup = setup(config(), None);
    attach(&setup).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert!(header_calls(&setup).is_empty());
    assert!(setup.fake.sent("Network.enable").is_empty());
}

#[tokio::test]
async fn a_live_change_sends_only_its_group_and_never_reloads() {
    let setup = setup(config(), None);
    attach(&setup).await;
    let light = EmulationFields {
        color_scheme: Some(ColorScheme::Light),
        ..EmulationFields::default()
    };
    let reload = setup
        .host
        .set_emulation_fields(ViewId(1), &light)
        .expect("sets");
    assert!(!reload);
    until(|| setup.fake.sent("Emulation.setEmulatedMedia").len() == 2).await;
    assert_eq!(
        setup.fake.sent("Emulation.setEmulatedMedia")[1]["params"]["features"][0]["value"],
        "light"
    );
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(
        setup.fake.sent("Emulation.setTouchEmulationEnabled").len(),
        1
    );
    assert_eq!(
        setup.fake.sent("Emulation.setDeviceMetricsOverride").len(),
        1
    );
    assert!(
        !setup
            .ui
            .calls()
            .iter()
            .any(|call| matches!(call, UiCall::Reload(..))),
        "the page is not reloaded"
    );
}

#[tokio::test]
async fn a_reconnect_restores_the_live_emulation() {
    let setup = setup(config(), None);
    attach(&setup).await;
    let landscape = EmulationFields {
        orientation: Some(rb_config::Orientation::Landscape),
        ..EmulationFields::default()
    };
    setup
        .host
        .set_emulation_fields(ViewId(1), &landscape)
        .expect("sets");
    until(|| setup.fake.sent("Emulation.setDeviceMetricsOverride").len() == 2).await;
    setup.fake.emit(json!({
        "method": "Target.detachedFromTarget",
        "params": { "sessionId": "S1", "targetId": "T1" }
    }));
    until(|| setup.fake.sent("Emulation.setLocaleOverride").len() == 2).await;
    let metrics = setup.fake.sent("Emulation.setDeviceMetricsOverride");
    let last = metrics.last().expect("metrics");
    assert_eq!(last["sessionId"], "S2");
    assert_eq!(last["params"]["width"], 852);
    assert_eq!(
        last["params"]["screenOrientation"]["type"],
        "landscapePrimary"
    );
    assert_eq!(
        setup
            .fake
            .sent("Emulation.setEmulatedMedia")
            .last()
            .expect("media")["sessionId"],
        "S2"
    );
    let info = setup.host.view(ViewId(1)).expect("the view");
    assert_eq!(info.width, 852);
}

/// One breakpoint with a position (RB-080-10).
fn geolocated() -> ProjectConfig {
    serde_json::from_value(json!({
        "version": 1,
        "breakpoints": [{
            "id": "mobile", "width": 393, "height": 852, "locale": "de-DE",
            "geolocation": { "latitude": 52.52, "longitude": 13.4 }
        }]
    }))
    .expect("a config")
}

fn grants(setup: &Setup) -> Vec<Value> {
    setup.fake.sent("Browser.grantPermissions")
}

/// The grant names the view's browser context, read from its target at the attach (charge-4
/// review, finding 4).
#[tokio::test]
async fn the_geolocation_grant_names_the_views_browser_context() {
    let setup = setup(geolocated(), None);
    setup.fake.answer_with(
        "Target.getTargetInfo",
        json!({ "targetInfo": {
            "targetId": "T1", "type": "page", "url": marker_url(ViewId(1)),
            "browserContextId": "CTX-1"
        } }),
    );
    attach(&setup).await;
    setup.host.on_address_change(ViewId(1), "https://a.test/x");
    until(|| !grants(&setup).is_empty()).await;
    assert_eq!(
        grants(&setup)[0]["params"],
        json!({
            "permissions": ["geolocation"],
            "origin": "https://a.test",
            "browserContextId": "CTX-1"
        })
    );
    until(|| setup.host.state().views[&ViewId(1)].geo_origin.is_some()).await;
}

/// A grant the browser refused is not remembered as granted, so the next apply sends it again
/// (charge-4 review, finding 4).
#[tokio::test]
async fn a_refused_grant_is_not_remembered_and_goes_again() {
    let setup = setup(geolocated(), None);
    setup.fake.fail("Browser.grantPermissions", true);
    attach(&setup).await;
    setup.host.on_address_change(ViewId(1), "https://a.test/x");
    until(|| !grants(&setup).is_empty()).await;
    tokio::time::sleep(Duration::from_millis(30)).await;
    assert_eq!(setup.host.state().views[&ViewId(1)].geo_origin, None);
    setup.fake.fail("Browser.grantPermissions", false);
    setup.host.reapply_emulation(ViewId(1));
    until(|| grants(&setup).len() == 2).await;
    until(|| setup.host.state().views[&ViewId(1)].geo_origin.as_deref() == Some("https://a.test"))
        .await;
}

#[tokio::test]
async fn a_session_lost_while_the_window_closes_is_not_attached_again() {
    // RB-080-91: the owner's log - `the main window is asked to close`, then `CDP session lost,
    // attaching again` for every view and `CDP attach failed ... the browser is not available`.
    let setup = setup(config(), None);
    attach(&setup).await;
    assert_eq!(setup.fake.sent("Target.attachToTarget").len(), 1);
    setup.host.begin_shutdown();
    setup.fake.emit(json!({
        "method": "Target.detachedFromTarget",
        "params": { "sessionId": "S1", "targetId": "T1" }
    }));
    // The event is handled: the view lost its session.
    until(|| {
        setup
            .host
            .state()
            .views
            .get(&ViewId(1))
            .is_some_and(|view| view.session.is_none())
    })
    .await;
    // Some time for an attach that should not come.
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(setup.fake.sent("Target.attachToTarget").len(), 1);
    assert!(
        !setup.host.state().views[&ViewId(1)].attaching,
        "no attach under way"
    );
    // A load that ends now attaches nothing either.
    setup.host.on_load_end(ViewId(1), &marker_url(ViewId(1)));
    for _ in 0..10 {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(setup.fake.sent("Target.attachToTarget").len(), 1);
}
