use super::*;
use serde_json::json;

fn config(value: serde_json::Value) -> ProjectConfig {
    serde_json::from_value(value).expect("a config")
}

#[test]
fn default_comes_first_and_is_listed_once() {
    let config = config(json!({ "version": 1, "sessions": ["admin", "default", "qa"] }));
    assert_eq!(known_sessions(&config), ["default", "admin", "qa"]);
    let empty = config_without_sessions();
    assert_eq!(known_sessions(&empty), ["default"]);
}

fn config_without_sessions() -> ProjectConfig {
    config(json!({ "version": 1 }))
}

#[test]
fn only_listed_sessions_and_default_are_known() {
    let config = config(json!({ "version": 1, "sessions": ["admin"] }));
    assert_eq!(require_known(&config, "default"), Ok(()));
    assert_eq!(require_known(&config, "admin"), Ok(()));
    assert_eq!(
        require_known(&config, "ghost"),
        Err(SessionError::Unknown("ghost".to_owned()))
    );
    assert_eq!(
        require_known(&config, "../admin"),
        Err(SessionError::InvalidId("../admin".to_owned()))
    );
}

#[test]
fn the_runtime_choice_wins_over_the_breakpoint() {
    assert_eq!(effective_session(None, None), "default");
    assert_eq!(effective_session(None, Some("admin")), "admin");
    assert_eq!(effective_session(Some("qa"), Some("admin")), "qa");
    assert_eq!(effective_session(Some("default"), Some("admin")), "default");
}

#[test]
fn default_has_no_badge_and_named_sessions_have_one() {
    let config = config(json!({
        "version": 1,
        "sessions": ["admin", "qa"],
        "sessionColors": { "admin": "#e5484d" }
    }));
    assert_eq!(badge(&config, "default"), None);
    assert_eq!(
        badge(&config, "admin"),
        Some(SessionBadge {
            name: "admin".to_owned(),
            rgb: 0xe5_48_4d
        })
    );
    let qa = badge(&config, "qa").expect("a badge");
    assert_eq!(qa.rgb, auto_color("qa"));
}

#[test]
fn automatic_colours_are_stable_and_from_the_palette() {
    assert_eq!(auto_color("admin"), auto_color("admin"));
    assert!(PALETTE.contains(&auto_color("admin")));
    assert!(PALETTE.contains(&auto_color("qa")));
    let distinct: std::collections::HashSet<u32> = ["a", "b", "c", "d", "e", "f", "g", "h"]
        .iter()
        .map(|id| auto_color(id))
        .collect();
    assert!(distinct.len() > 2, "the palette is used: {distinct:?}");
}

#[test]
fn colours_render_as_css_and_argb() {
    assert_eq!(css_color(0xe5_48_4d), "#e5484d");
    assert_eq!(css_color(0x00_00_0f), "#00000f");
    assert_eq!(argb(0xe5_48_4d), 0xff_e5_48_4d);
}

#[test]
fn the_badge_text_starts_with_a_dot() {
    let badge = SessionBadge {
        name: "admin".to_owned(),
        rgb: 0,
    };
    assert_eq!(badge.text(), "\u{25cf} admin");
}
