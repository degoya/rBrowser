use std::sync::Arc;

use rb_ipc::BoundaryKind;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::ui::recording::RecordingUi;

fn host() -> Arc<Host> {
    let (_fake, router) = FakeBrowser::connect();
    Host::new(Arc::new(RecordingUi::default()), router, Handle::current())
}

#[tokio::test]
async fn a_change_reaches_the_subscribers_once_and_no_change_not_at_all() {
    let host = host();
    let mut events = rb_engine::ViewHost::subscribe(host.as_ref());
    assert_eq!(host.shell_state(), ShellState::at_open());
    host.update_shell_state(|state| {
        state.trusted = true;
        state.detected.push(DetectedBreakpoint {
            width: 768,
            kind: BoundaryKind::Min,
            label: Some("md".to_owned()),
            count: 3,
            source: None,
            heuristic: false,
        });
    });
    host.update_shell_state(|state| state.trusted = true);
    assert!(host.shell_state().trusted);
    assert_eq!(host.shell_state().detected[0].width, 768);
    assert_eq!(events.try_recv().ok(), Some(ViewEvent::StateChanged));
    assert!(
        events.try_recv().is_err(),
        "an unchanged state sends nothing"
    );
}

/// RB-099-26: the built-in mode is the canvas, but a new window shows the grid the host builds
/// from the config until the start selection is applied - the state says what the views are in.
#[tokio::test]
async fn a_new_window_is_in_the_grid_until_the_start_selection() {
    assert_eq!(LayoutMode::default(), LayoutMode::Canvas);
    let host = host();
    let shell = host.shell_state();
    assert_eq!(shell.mode, LayoutMode::Grid);
    assert_eq!(shell, ShellState::at_open());
}
