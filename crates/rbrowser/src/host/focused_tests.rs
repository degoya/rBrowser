use rb_config::{FocusModeSettings, LayoutMode};
use rb_engine::ViewHost;

use super::{FocusRequest, keeps_focus};
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup, view_ids};

/// The project with `sync.activateOnFocus` on.
fn focus_activates() -> rb_config::ProjectConfig {
    let mut project = config();
    project.sync.activate_on_focus = true;
    project
}

fn focus_calls(calls: &[UiCall]) -> usize {
    calls
        .iter()
        .filter(|call| matches!(call, UiCall::Focus(_)))
        .count()
}

/// A click into a view that is not the active one makes it the active view (the leader).
#[tokio::test]
async fn a_focused_view_becomes_the_active_view() {
    let setup = setup(focus_activates());
    let host = setup.core.host();
    let ids = view_ids(&setup);
    host.set_active_view(ids[0]).expect("activates");
    assert!(host.on_view_focused(ids[1]));
    assert_eq!(host.active_view(), Some(ids[1]));
}

/// The view that got the focus becomes active without the host focusing it again: a focus
/// queued for it would reach the window after the next view's and take the focus back, and the
/// views took the lead from each other thousands of times a second (diag4, the canvas labels
/// flashing).
#[tokio::test]
async fn a_focused_view_becomes_active_without_being_focused_again() {
    let setup = setup(focus_activates());
    let host = setup.core.host();
    let ids = view_ids(&setup);
    host.set_active_view(ids[0]).expect("activates");
    let before = focus_calls(&setup.ui.calls());
    assert!(host.on_view_focused(ids[1]));
    assert!(host.on_view_focused(ids[2]));
    assert_eq!(focus_calls(&setup.ui.calls()), before, "no focus queued");
    assert_eq!(host.active_view(), Some(ids[2]));
}

/// A new active view - a label click or the focus - is marked in the window once; activating it
/// again marks nothing.
#[tokio::test]
async fn a_new_active_view_is_marked_in_the_window_once() {
    let setup = setup(focus_activates());
    let host = setup.core.host();
    let ids = view_ids(&setup);
    host.set_active_view(ids[1]).expect("activates");
    host.set_active_view(ids[1]).expect("activates");
    assert!(host.on_view_focused(ids[2]));
    let marks: Vec<UiCall> = setup
        .ui
        .calls()
        .into_iter()
        .filter(|call| matches!(call, UiCall::MarkActive(_)))
        .collect();
    assert_eq!(
        marks,
        [
            UiCall::MarkActive(Some(ids[1])),
            UiCall::MarkActive(Some(ids[2]))
        ]
    );
}

/// The active view getting the focus (set_active_view focuses it itself) changes nothing and
/// focuses nothing again: no loop.
#[tokio::test]
async fn the_active_view_getting_the_focus_does_nothing() {
    let setup = setup(focus_activates());
    let host = setup.core.host();
    let ids = view_ids(&setup);
    host.set_active_view(ids[2]).expect("activates");
    let before = focus_calls(&setup.ui.calls());
    assert!(!host.on_view_focused(ids[2]));
    assert_eq!(focus_calls(&setup.ui.calls()), before);
    assert_eq!(host.active_view(), Some(ids[2]));
}

/// By default (`sync.activateOnFocus` off) only the label makes a view active: a focused view
/// stays a follower.
#[tokio::test]
async fn by_default_a_focused_view_stays_a_follower() {
    let setup = setup(config());
    let host = setup.core.host();
    let ids = view_ids(&setup);
    host.set_active_view(ids[0]).expect("activates");
    assert!(!host.on_view_focused(ids[1]));
    assert_eq!(host.active_view(), Some(ids[0]));
}

/// A view the host does not know (closed a moment ago) is ignored.
#[tokio::test]
async fn an_unknown_view_is_ignored() {
    let setup = setup(focus_activates());
    let host = setup.core.host();
    let active = host.active_view();
    assert!(!host.on_view_focused(rb_engine::ViewId(999)));
    assert_eq!(host.active_view(), active);
}

/// In focus mode a page loading in a view - each arrow key of the toolbar opens a new view for
/// the next breakpoint - leaves the keyboard focus where it is, so the next arrow key steps
/// again (RB-080-17). A click into the view or an activation always gets the focus.
#[test]
fn in_focus_mode_a_loading_page_leaves_the_focus_where_it_is() {
    let settings = FocusModeSettings::default();
    assert!(settings.keep_focus, "on by default");
    assert!(keeps_focus(
        FocusRequest::Navigation,
        LayoutMode::Focus,
        &settings
    ));
    assert!(!keeps_focus(
        FocusRequest::System,
        LayoutMode::Focus,
        &settings
    ));
}

/// Outside focus mode, and with `focusMode.keepFocus` off, a loading page takes the focus as it
/// did before.
#[test]
fn elsewhere_a_loading_page_takes_the_focus() {
    let settings = FocusModeSettings::default();
    for mode in [LayoutMode::Grid, LayoutMode::Horizontal, LayoutMode::Canvas] {
        assert!(
            !keeps_focus(FocusRequest::Navigation, mode, &settings),
            "{mode:?}"
        );
    }
    let off = FocusModeSettings {
        keep_focus: false,
        ..FocusModeSettings::default()
    };
    assert!(!keeps_focus(
        FocusRequest::Navigation,
        LayoutMode::Focus,
        &off
    ));
}

/// RB-090-38: the config in effect is read under the lock, the same as its copy.
#[tokio::test]
async fn with_config_reads_the_config_in_effect() {
    let setup = setup(focus_activates());
    let host = setup.core.host();
    assert!(host.with_config(|config| config.sync.activate_on_focus));
    assert_eq!(
        host.with_config(|config| config.breakpoints.clone()),
        host.config().breakpoints
    );
}
