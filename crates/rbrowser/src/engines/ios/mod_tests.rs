use std::sync::Arc;
use std::time::Duration;

use rb_config::{EngineKind, IosSettings};
use rb_engine::ViewId;
use rb_engine::emulation_report::LEADER_FIELD;
use rb_engine::external::ExternalStatus;
use rb_safari::driver::Attached;
use rb_safari::ios::fake::{FakeIosTools, SIMULATOR};
use rb_safari::ios::{IosMode, IosTools};
use rb_safari::testing::FakeSafari;
use rb_sync::ViewRoute;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::engines::tests::until;
use crate::host::nav::marker_url;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// The default window with its mobile view (3) on iOS.
pub(super) fn setup_on(runtime: Handle) -> Arc<Core> {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, runtime.clone());
    let mut config = default_config();
    config.breakpoints[2].engine = EngineKind::Ios;
    host.open_config(config, None).expect("opens");
    for id in 1..=3 {
        fake.add_target(&format!("T{id}"), &marker_url(ViewId(id)));
    }
    Core::new(host, ui, runtime, CoreSetup::default())
}

fn status(core: &Core) -> Option<(ExternalStatus, Option<String>)> {
    crate::engines::for_host(core.host())?
        .list()
        .into_iter()
        .find(|view| view.view == ViewId(3))
        .map(|view| (view.status, view.reason))
}

/// The window started with iOS over `fake` and `tools`.
pub(super) fn start(core: &Core, fake: &FakeSafari, tools: &Arc<FakeIosTools>) {
    crate::sync_hub::on_start(core);
    let setup = IosSetup {
        launch: Arc::new(Attached(fake.port())),
        tools: tools.clone() as Arc<dyn IosTools>,
    };
    crate::engines::start_engines_with(core, None, None, Some(setup));
}

#[test]
fn the_settings_become_the_engine_s_configuration() {
    let settings = IosSettings {
        mode: "device".to_owned(),
        default_simulator: "iPad Air 11-inch (M2)".to_owned(),
        boot_timeout_sec: 45,
    };
    let config = config_of(&settings);
    assert_eq!(config.mode, IosMode::Device);
    assert_eq!(config.default_simulator, "iPad Air 11-inch (M2)");
    assert_eq!(config.boot_timeout, Duration::from_secs(45));
    // A boot wait out of the schema's range is held to it.
    let config = config_of(&IosSettings {
        boot_timeout_sec: 0,
        ..IosSettings::default()
    });
    assert_eq!(config.boot_timeout, Duration::from_secs(10));
}

#[test]
fn only_a_mac_has_ios_views() {
    assert_eq!(app_setup().is_some(), cfg!(target_os = "macos"));
}

#[test]
fn a_device_name_is_short_printable_text() {
    assert!(valid_device("iPhone 16"));
    assert!(valid_device("iPad Pro 13-inch (M4)"));
    assert!(valid_device(SIMULATOR));
    assert!(!valid_device(""));
    assert!(!valid_device("   "));
    assert!(!valid_device("iPhone\n16"));
    assert!(!valid_device(&"x".repeat(81)));
}

#[tokio::test(flavor = "multi_thread")]
async fn an_ios_view_runs_on_the_simulator_and_the_sync_routes_to_it() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    fake.screen(390, 844);
    let tools = Arc::new(FakeIosTools::with_xcode());
    start(&core, &fake, &tools);
    until(|| status(&core).is_some_and(|(status, _)| status == ExternalStatus::Running)).await;
    assert_eq!(tools.booted(), [SIMULATOR]);
    let sync = crate::sync_hub::for_host(core.host()).expect("sync runs");
    until(|| sync.router().route(ViewId(3)) == ViewRoute::Engine(EngineKind::Ios)).await;
    let engines = crate::engines::for_host(core.host()).expect("started");
    let view = &engines.list()[0];
    assert_eq!(view.engine, EngineKind::Ios);
    assert!(
        view.unsupported.iter().any(|field| field == LEADER_FIELD),
        "an iOS view only follows: {:?}",
        view.unsupported
    );
    assert!(for_host(core.host()).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_ios_view_follows_the_navigation_of_its_cell() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    let tools = Arc::new(FakeIosTools::with_device());
    start(&core, &fake, &tools);
    until(|| status(&core).is_some_and(|(status, _)| status == ExternalStatus::Running)).await;
    core.host()
        .on_address_change(ViewId(3), "http://localhost/from-grid");
    until(|| {
        fake.requests_to("/url").iter().any(|request| {
            request.method == "POST" && request.body["url"] == "http://localhost/from-grid"
        })
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn without_xcode_and_without_a_device_the_view_names_both_ways() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    let tools = Arc::new(FakeIosTools::default());
    start(&core, &fake, &tools);
    let reason = rb_safari::ios::NoTarget::NoWay.reason();
    until(|| status(&core).is_some_and(|(_, why)| why.as_deref() == Some(reason.as_str()))).await;
    assert_eq!(fake.sessions(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn without_a_mac_ios_is_unavailable_and_says_why() {
    let core = setup_on(Handle::current());
    crate::sync_hub::on_start(&core);
    crate::engines::start_engines_with(&core, None, None, None);
    let reason = rb_i18n::t("engines.ios.onlyMacos");
    until(|| status(&core) == Some((ExternalStatus::Unavailable, Some(reason.clone())))).await;
    let reason_of = crate::breakpoint_keys::engine_pick::unavailable_reason(&core, EngineKind::Ios);
    assert_eq!(reason_of, Some(reason));
}
