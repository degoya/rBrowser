use std::sync::Arc;

use rb_config::DevtoolsMode;
use rb_engine::ViewId;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::{Core, CoreSetup};
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

fn core_with(user: rb_config::UserSettings) -> Arc<Core> {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    Core::new(
        host,
        ui,
        Handle::current(),
        CoreSetup {
            user,
            ..CoreSetup::default()
        },
    )
}

#[tokio::test]
async fn the_state_reads_the_setting_the_view_and_the_registry() {
    let core = core_with(rb_config::UserSettings::default());
    let state = state_for(&core, ViewId(1), Some((3, 4)));
    assert_eq!(state.view, ViewId(1));
    assert_eq!(state.point, Some((3, 4)));
    assert!(state.menu_enabled, "ui.pageContextMenu is on by default");
    assert!(!state.attached, "no CDP session before the attach");
    assert!(!state.detached);
    assert!(!state.devtools_open);
    assert!(
        state.open_on_inspect,
        "devtools.openOnInspect is on by default"
    );
    assert_eq!(state.recording, None);
    assert!(!state.checks_busy);
    assert!(state.checks.is_empty(), "no plugin runtime in this window");
    assert!(state.panels.is_empty());
    for known in ["breakpoints.analyze", "panels.sitemap"] {
        assert!(state.known.contains(known), "{known}");
    }
    assert!(
        state.known.contains("view.remove"),
        "RB-080-41's command offers Remove view"
    );
    // RB-090-60: the Recorder and Run flow submenus; no recording runs.
    for known in ["recorder.check", "flows.run"] {
        assert!(state.known.contains(known), "{known}");
    }
    assert_eq!(state.recorder.recording, None);
    let mut user = rb_config::UserSettings::default();
    user.ui.page_context_menu = false;
    user.devtools.open_on_inspect = false;
    user.ui.page_context_menu_emulation = false;
    let off = state_for(&core_with(user), ViewId(1), None);
    assert_eq!(off.point, None);
    assert!(!off.menu_enabled);
    assert!(!off.open_on_inspect);
    assert_eq!(off.emulation, None);
}

#[tokio::test]
async fn devtools_count_as_open_for_their_view_only() {
    let core = core_with(rb_config::UserSettings::default());
    let devtools = crate::devtools::start(&core, Handle::current(), None);
    devtools
        .open(ViewId(2), DevtoolsMode::Window)
        .expect("opens");
    assert!(state_for(&core, ViewId(2), None).devtools_open);
    assert!(!state_for(&core, ViewId(1), None).devtools_open);
}

/// The Emulation submenu reads the clicked view's live emulation.
#[tokio::test]
async fn the_emulation_state_is_the_clicked_views() {
    use rb_engine::ViewHost as _;
    let core = core_with(rb_config::UserSettings::default());
    core.run_command(
        "emulation.set",
        serde_json::json!({ "target": 2, "fields": { "colorScheme": "dark", "reducedMotion": true } }),
    )
    .expect("sets");
    assert!(core.host().emulation(ViewId(2)).is_some());
    let clicked = state_for(&core, ViewId(2), None).emulation.expect("on");
    assert_eq!(clicked.color_scheme, Some(rb_config::ColorScheme::Dark));
    assert!(clicked.reduced_motion);
    assert_eq!(clicked.all.color_scheme, None, "the views differ");
    assert!(!clicked.all.reduced_motion);
    let other = state_for(&core, ViewId(1), None).emulation.expect("on");
    assert_eq!(other.color_scheme, None);
    assert!(!other.reduced_motion);
    core.run_command(
        "emulation.set",
        serde_json::json!({ "target": "all", "fields": { "colorScheme": "dark", "reducedMotion": true } }),
    )
    .expect("sets");
    let all = state_for(&core, ViewId(1), None).emulation.expect("on").all;
    assert_eq!(all.color_scheme, Some(Some(rb_config::ColorScheme::Dark)));
    assert!(all.reduced_motion);
    // `colorScheme: null` drops the override of the one view, and only it.
    core.run_command(
        "emulation.set",
        serde_json::json!({ "target": 2, "fields": { "colorScheme": null } }),
    )
    .expect("clears");
    let cleared = state_for(&core, ViewId(2), None).emulation.expect("on");
    assert_eq!(cleared.color_scheme, None);
    assert!(cleared.reduced_motion, "the other overrides stay");
    assert_eq!(cleared.all.color_scheme, None, "mixed again");
}

/// RB-080-60: the footer entry knows its command and checks what `ui.viewFooter.visible` says.
#[tokio::test]
async fn the_state_reads_the_footer_switch() {
    let core = core_with(rb_config::UserSettings::default());
    let state = state_for(&core, ViewId(1), None);
    assert!(state.known.contains(crate::view_footer::defs::TOGGLE));
    assert!(state.footer_visible, "the footer is on by default");
    let mut user = rb_config::UserSettings::default();
    user.ui.view_footer.visible = false;
    assert!(!state_for(&core_with(user), ViewId(1), None).footer_visible);
}

/// RB-090-71: the vitals bar's entry knows its command and checks `ui.viewFooter.vitals`.
#[tokio::test]
async fn the_state_reads_the_vitals_bar_switch() {
    let core = core_with(rb_config::UserSettings::default());
    let state = state_for(&core, ViewId(1), None);
    assert!(
        state
            .known
            .contains(crate::view_footer::defs::VITALS_TOGGLE)
    );
    assert!(state.vitals_bar_visible, "the vitals bar is on by default");
    let mut user = rb_config::UserSettings::default();
    user.ui.view_footer.vitals = false;
    let off = state_for(&core_with(user), ViewId(1), None);
    assert!(!off.vitals_bar_visible);
    assert!(off.footer_visible, "each bar on its own");
}

/// RB-090-73: the Lighthouse bar's entry knows its command and checks
/// `ui.viewFooter.lighthouse`.
#[tokio::test]
async fn the_state_reads_the_lighthouse_bar_switch() {
    let core = core_with(rb_config::UserSettings::default());
    let state = state_for(&core, ViewId(1), None);
    assert!(
        state
            .known
            .contains(crate::view_footer::defs::LIGHTHOUSE_TOGGLE)
    );
    assert!(state.lighthouse_bar_visible, "on by default");
    let mut user = rb_config::UserSettings::default();
    user.ui.view_footer.lighthouse = false;
    let off = state_for(&core_with(user), ViewId(1), None);
    assert!(!off.lighthouse_bar_visible);
    assert!(off.vitals_bar_visible, "each bar on its own");
}
