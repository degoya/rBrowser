use std::sync::Arc;

use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::state::default_config;
use crate::host::ui::recording::RecordingUi;

/// RB-099-19: after a lag the views the host has decide - one it closed meanwhile is
/// forgotten, its own are kept.
#[tokio::test(flavor = "multi_thread")]
async fn a_lag_forgets_the_views_the_host_no_longer_has() {
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(
        Arc::new(RecordingUi::default()),
        router.clone(),
        Handle::current(),
    );
    host.open_config(default_config(), None).expect("opens");
    let hooks = TimeHooks::default();
    for (view, session) in [(1, "S1"), (2, "S2"), (99, "S99")] {
        hooks.attach(&router, ViewId(view), session).await;
    }
    assert!(reconcile(&host, &hooks), "view 99 is gone");
    assert_eq!(hooks.rates(), [(ViewId(1), 1.0), (ViewId(2), 1.0)]);
    assert!(!reconcile(&host, &hooks));
}
