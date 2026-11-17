use rb_engine::{Target, ViewId};
use serde_json::json;

use super::*;

#[test]
fn the_arguments_default_to_a_full_reload_of_every_view() {
    assert_eq!(
        ReloadArgs::from_value(&serde_json::Value::Null),
        Ok(ReloadArgs::default())
    );
    assert_eq!(
        ReloadArgs::from_value(&json!({})),
        Ok(ReloadArgs {
            target: Target::All,
            mode: ReloadKind::Full,
            ignore_cache: false,
        })
    );
}

#[test]
fn the_arguments_read_a_view_a_mode_and_the_cache_flag() {
    assert_eq!(
        ReloadArgs::from_value(&json!({ "target": 3, "mode": "css", "ignoreCache": true })),
        Ok(ReloadArgs {
            target: Target::View(ViewId(3)),
            mode: ReloadKind::Css,
            ignore_cache: true,
        })
    );
}

#[test]
fn unknown_or_wrong_arguments_are_refused() {
    assert!(ReloadArgs::from_value(&json!({ "mode": "hard" })).is_err());
    assert!(ReloadArgs::from_value(&json!({ "target": "some" })).is_err());
    assert!(ReloadArgs::from_value(&json!({ "url": "x" })).is_err());
}

#[test]
fn the_answer_names_what_happened_per_view() {
    let answer = ReloadAnswer {
        mode: ReloadKind::Css,
        views: vec![
            ViewReload {
                view: ViewId(1),
                action: ViewAction::Swapped { count: 2 },
            },
            ViewReload {
                view: ViewId(2),
                action: ViewAction::LeftToHmr,
            },
        ],
    };
    assert_eq!(
        serde_json::to_value(&answer).expect("serialises"),
        json!({
            "mode": "css",
            "views": [
                { "view": 1, "action": "swapped", "count": 2 },
                { "view": 2, "action": "left_to_hmr" },
            ],
        })
    );
    assert!(TOOL_DESCRIPTION.contains("\"css\""));
    assert_eq!(TOOL_NAME, "reload");
}

#[test]
fn a_full_reload_is_left_to_navigation_sync_css_and_bad_arguments_are_not() {
    // RB-020-06: `reload.all` reaches the coupled views; hot reload keeps `mode: "css"`.
    assert!(ReloadArgs::is_full(&serde_json::Value::Null));
    assert!(ReloadArgs::is_full(
        &serde_json::json!({ "target": 2, "ignoreCache": true })
    ));
    assert!(!ReloadArgs::is_full(&serde_json::json!({ "mode": "css" })));
    assert!(!ReloadArgs::is_full(&serde_json::json!({ "mode": "nope" })));
    // Review batch 3, finding 3: the address field of a decoupled view sends `view`, which
    // navigation sync reads; hot reload's own arguments do not have it.
    assert!(ReloadArgs::is_full(&serde_json::json!({ "view": 2 })));
    assert!(ReloadArgs::is_full(
        &serde_json::json!({ "view": 2, "mode": "full", "ignoreCache": false })
    ));
    assert!(!ReloadArgs::is_full(&serde_json::json!("all")));
}
