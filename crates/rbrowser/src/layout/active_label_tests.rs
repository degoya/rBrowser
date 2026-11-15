use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup, view_ids};

#[tokio::test]
async fn the_window_gets_the_switch_and_the_active_view_at_the_start() {
    let setup = setup(config());
    let ids = view_ids(&setup);
    setup
        .core
        .host()
        .set_active_view(ids[1])
        .expect("activates");
    on_start(&setup.core);
    let calls = setup.ui.calls();
    assert!(
        calls.contains(&UiCall::ActiveHighlight(true)),
        "on by default"
    );
    assert_eq!(calls.last(), Some(&UiCall::MarkActive(Some(ids[1]))));
}

#[tokio::test]
async fn switched_off_the_window_marks_no_label() {
    let setup = setup(config());
    let mut user = setup.core.user_settings();
    user.ui.highlight_active_view = false;
    setup.core.replace_user_settings(user);
    assert!(setup.ui.calls().contains(&UiCall::ActiveHighlight(false)));
}
