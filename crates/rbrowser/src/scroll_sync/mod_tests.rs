use std::sync::Arc;

use rb_config::ScrollMode;
use tokio::runtime::{Builder, Handle, Runtime};

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// A window with the default config's views.
pub(crate) struct Setup {
    pub(crate) _fake: Arc<FakeBrowser>,
    pub(crate) core: Arc<Core>,
}

/// A window over `runtime`.
pub(crate) fn setup_on(runtime: &Handle) -> Setup {
    let (fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, runtime.clone());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host, ui, runtime.clone(), CoreSetup::default());
    Setup { _fake: fake, core }
}

/// A runtime of its own and a window on it, built inside it; the test itself then runs on a
/// plain thread, as the CEF UI thread runs the hooks.
fn outside_the_runtime() -> (Runtime, Setup) {
    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let setup = {
        let _inside = runtime.enter();
        setup_on(runtime.handle())
    };
    (runtime, setup)
}

#[test]
fn the_mode_starts_from_the_project_and_a_config_change_drops_the_runtime_one() {
    let (_runtime, setup) = outside_the_runtime();
    let host = setup.core.host();
    assert_eq!(mode(host), ScrollMode::Anchor);
    set_mode(host, Some(ScrollMode::Wheel));
    assert_eq!(mode(host), ScrollMode::Wheel);
    // Off the runtime, as on CEF's UI thread: the hook spawns nothing.
    on_config_changed(&setup.core);
    assert_eq!(mode(host), ScrollMode::Anchor);
}

#[test]
fn the_runtime_mode_is_per_window() {
    let (_runtime, first) = outside_the_runtime();
    let (_other, second) = outside_the_runtime();
    set_mode(first.core.host(), Some(ScrollMode::Wheel));
    assert_eq!(mode(first.core.host()), ScrollMode::Wheel);
    assert_eq!(mode(second.core.host()), ScrollMode::Anchor);
    set_mode(first.core.host(), Some(ScrollMode::Anchor));
    set_mode(first.core.host(), Some(ScrollMode::Wheel));
    assert_eq!(mode(first.core.host()), ScrollMode::Wheel);
    set_mode(first.core.host(), None);
    assert_eq!(mode(first.core.host()), ScrollMode::Anchor);
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_set_switches_the_scroll_mode_and_the_replay_follows() {
    use crate::replay::{HostPort, ReplayPort};
    use crate::sync_hub::mcp::sync_set;
    use serde_json::json;

    let setup = setup_on(&Handle::current());
    crate::sync_hub::on_start(&setup.core);
    let core = &setup.core;
    let port = HostPort::new(core.host());
    assert_eq!(port.scroll_mode(), ScrollMode::Anchor);
    let state = sync_set(core, &json!({ "scrollMode": "wheel" })).expect("switched");
    assert_eq!(state.features.scroll_mode, ScrollMode::Wheel);
    assert!(state.features.scroll, "the switches stay as they were");
    assert_eq!(port.scroll_mode(), ScrollMode::Wheel);
    let state = sync_set(core, &json!({ "feature": "scroll", "enabled": false })).expect("off");
    assert!(!state.features.scroll && state.features.click);
    assert_eq!(state.features.scroll_mode, ScrollMode::Wheel);
    assert!(sync_set(core, &json!({ "scrollMode": "sideways" })).is_err());
    assert!(
        sync_set(core, &json!({})).is_err(),
        "`set` still needs `enabled`"
    );
}
