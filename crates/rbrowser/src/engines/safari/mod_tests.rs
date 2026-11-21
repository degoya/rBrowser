use std::sync::Arc;

use rb_config::{EngineKind, SafariSettings};
use rb_engine::ViewId;
use rb_engine::emulation_report::LEADER_FIELD;
use rb_engine::external::ExternalStatus;
use rb_safari::driver::Attached;
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

/// The default window with its mobile view (3) on Safari.
fn setup_on(runtime: Handle) -> Arc<Core> {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, runtime.clone());
    let mut config = default_config();
    config.breakpoints[2].engine = EngineKind::Safari;
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

fn running(core: &Core) -> bool {
    status(core).is_some_and(|(status, _)| status == ExternalStatus::Running)
}

/// The window started with Safari over `fake`.
fn start(core: &Core, fake: &FakeSafari) {
    crate::sync_hub::on_start(core);
    let launch: Arc<dyn rb_safari::Launch> = Arc::new(Attached(fake.port()));
    crate::engines::start_engines(core, None, Some(launch));
}

#[test]
fn the_settings_choose_the_edition_and_switch_safari() {
    let settings = SafariSettings {
        enabled: false,
        technology_preview: true,
    };
    assert_eq!(
        config_of(&settings),
        SafariConfig {
            enabled: false,
            edition: Edition::TechnologyPreview,
        }
    );
    assert_eq!(
        config_of(&SafariSettings::default()),
        SafariConfig::default()
    );
}

#[test]
fn only_a_mac_has_a_safari_driver() {
    assert_eq!(app_launch().is_some(), cfg!(target_os = "macos"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_safari_view_runs_in_its_window_and_the_sync_routes_to_it() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    start(&core, &fake);
    until(|| running(&core)).await;
    assert_eq!(fake.sessions(), 1);
    let sync = crate::sync_hub::for_host(core.host()).expect("sync runs");
    until(|| sync.router().route(ViewId(3)) == ViewRoute::Engine(EngineKind::Safari)).await;
    let engines = crate::engines::for_host(core.host()).expect("started");
    let view = &engines.list()[0];
    assert_eq!(view.engine, EngineKind::Safari);
    // views_list says the view only follows.
    assert!(
        view.unsupported.iter().any(|field| field == LEADER_FIELD),
        "{:?}",
        view.unsupported
    );
    assert!(for_host(core.host()).is_some());
}

#[tokio::test(flavor = "multi_thread")]
async fn the_safari_window_follows_its_cell() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    start(&core, &fake);
    until(|| running(&core)).await;
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
async fn without_a_mac_safari_is_unavailable_and_says_why() {
    let core = setup_on(Handle::current());
    crate::sync_hub::on_start(&core);
    crate::engines::start_engines(&core, None, None);
    let reason = rb_i18n::t("engines.safari.onlyMacos");
    until(|| status(&core) == Some((ExternalStatus::Unavailable, Some(reason.clone())))).await;
    // The badge menu offers the row disabled with that reason.
    let reason_of =
        crate::breakpoint_keys::engine_pick::unavailable_reason(&core, EngineKind::Safari);
    assert_eq!(reason_of, Some(reason));
}

#[tokio::test(flavor = "multi_thread")]
async fn remote_automation_off_shows_the_command_in_the_view_s_reason() {
    let core = setup_on(Handle::current());
    let fake = FakeSafari::start().await;
    fake.refuse_sessions(
        "Could not create a session: You must enable the 'Allow Remote Automation' option in Safari's Develop menu to control Safari via WebDriver.",
    );
    start(&core, &fake);
    let reason = rb_i18n::t("engines.safari.notEnabled");
    until(|| {
        status(&core).is_some_and(|(status, why)| {
            status == ExternalStatus::Failed && why.as_deref() == Some(reason.as_str())
        })
    })
    .await;
}
