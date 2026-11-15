use rb_config::EngineKind;
use rb_engine::ViewId;
use serde_json::json;

use super::*;

#[test]
fn engine_badge_lists_the_three_engines_with_the_current_one_checked() {
    let items = items(EngineKind::Firefox, |_| None);
    let engines: Vec<EngineKind> = items.iter().map(|item| item.engine).collect();
    assert_eq!(
        engines,
        [
            EngineKind::Chromium,
            EngineKind::Firefox,
            EngineKind::Webkit
        ]
    );
    let checked: Vec<bool> = items.iter().map(|item| item.checked).collect();
    assert_eq!(checked, [false, true, false]);
    assert!(items.iter().all(EngineItem::available));
    assert_eq!(items[0].label, "Chromium");
    assert_eq!(items[1].label, "Firefox");
    assert!(items[2].label.starts_with("WebKit"), "{}", items[2].label);
}

#[test]
fn engine_badge_disables_an_engine_that_cannot_run_and_names_the_reason() {
    let items = items(EngineKind::Chromium, |engine| {
        (engine == EngineKind::Firefox).then(|| "Firefox was not found".to_owned())
    });
    assert!(items[0].available());
    assert!(!items[1].available());
    assert!(items[2].available());
    assert!(
        items[1].label == ROW_UNAVAILABLE
            || (items[1].label.contains("Firefox")
                && items[1].label.contains("Firefox was not found")),
        "{}",
        items[1].label
    );
    assert_eq!(items[1].reason.as_deref(), Some("Firefox was not found"));
}

#[test]
fn engine_badge_names_the_engine_and_dims_with_the_reason() {
    let running = badge(EngineKind::Webkit, None);
    assert_eq!(running.text, "WebKit");
    assert!(running.available());
    assert!(
        running.tooltip == BADGE_TOOLTIP || running.tooltip.contains("WebKit"),
        "{}",
        running.tooltip
    );
    let missing = badge(EngineKind::Firefox, Some("not installed".to_owned()));
    assert_eq!(missing.text, "Firefox");
    assert!(!missing.available());
    // The text comes with the wave's locale part; until it is merged `t_with` answers the key.
    assert!(
        missing.tooltip == BADGE_UNAVAILABLE
            || (missing.tooltip.contains("not installed") && missing.tooltip.contains("Chromium")),
        "{}",
        missing.tooltip
    );
}

#[test]
fn engine_badge_command_args_name_the_view_and_the_engine() {
    let items = items(EngineKind::Chromium, |_| None);
    assert_eq!(
        command_args(&items[2], ViewId(4)),
        json!({ "view": 4, "engine": "webkit" })
    );
    assert_eq!(config_name(EngineKind::Firefox), "firefox");
    assert_eq!(name_key(EngineKind::Chromium), "engines.name.chromium");
}

#[tokio::test]
async fn engine_badge_reads_the_views_engine_and_runs_nothing_for_a_disabled_row() {
    let setup = crate::layout::test_support::setup(crate::layout::test_support::config());
    let core = &setup.core;
    let ids = crate::layout::test_support::view_ids(&setup);
    let badge = badge_of(core, ids[0]).expect("the view exists");
    assert_eq!(
        (badge.engine, badge.text.as_str()),
        (EngineKind::Chromium, "Chromium")
    );
    assert!(badge.available(), "no external windows: nothing says no");
    assert!(badge_of(core, ViewId(999)).is_none());
    let items = engine_pick(core, ids[0]).expect("the view exists");
    assert!(items[0].checked && !items[1].checked);
    let disabled = EngineItem {
        reason: Some("gone".to_owned()),
        ..items[1].clone()
    };
    pick_engine(core, ids[0], &disabled);
    assert_eq!(
        core.host().view(ids[0]).expect("exists").engine,
        EngineKind::Chromium
    );
}
