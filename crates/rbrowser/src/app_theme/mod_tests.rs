use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::test_support::{config, setup};

#[test]
fn the_turn_goes_system_light_dark_and_round() {
    assert_eq!(next(Theme::System), Theme::Light);
    assert_eq!(next(Theme::Light), Theme::Dark);
    assert_eq!(next(Theme::Dark), Theme::System);
}

#[test]
fn the_words_are_the_settings() {
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        assert_eq!(parse(word(theme)), Some(theme));
        assert_eq!(
            serde_json::to_value(theme).expect("json"),
            serde_json::json!(word(theme))
        );
    }
    assert_eq!(parse("Dark"), None);
    assert_eq!(parse(""), None);
}

#[tokio::test]
async fn the_window_takes_the_theme_at_the_start() {
    let setup = setup(config());
    on_start(&setup.core);
    assert_eq!(
        setup.ui.calls().last(),
        Some(&UiCall::AppTheme(Theme::System))
    );
}

#[tokio::test]
async fn the_window_follows_a_changed_setting_without_a_restart() {
    let setup = setup(config());
    let mut user = setup.core.user_settings();
    user.ui.theme = Theme::Dark;
    setup.core.replace_user_settings(user);
    assert!(setup.ui.calls().contains(&UiCall::AppTheme(Theme::Dark)));
    let mut user = setup.core.user_settings();
    user.ui.theme = Theme::Light;
    setup.core.replace_user_settings(user);
    assert_eq!(
        setup
            .ui
            .calls()
            .iter()
            .rev()
            .find(|call| matches!(call, UiCall::AppTheme(_))),
        Some(&UiCall::AppTheme(Theme::Light))
    );
}
