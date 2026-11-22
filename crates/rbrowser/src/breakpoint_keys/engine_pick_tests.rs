use rb_config::EngineKind;
use rb_engine::ViewId;
use serde_json::json;

use super::*;

#[test]
fn engine_badge_lists_the_engines_with_the_current_one_checked() {
    let items = items(EngineKind::Firefox, |_| None);
    let engines: Vec<EngineKind> = items.iter().map(|item| item.engine).collect();
    assert_eq!(
        engines,
        [
            EngineKind::Chromium,
            EngineKind::Firefox,
            EngineKind::Webkit,
            EngineKind::Safari,
            EngineKind::Ios
        ]
    );
    let checked: Vec<bool> = items.iter().map(|item| item.checked).collect();
    assert_eq!(checked, [false, true, false, false, false]);
    assert!(items.iter().all(|item| item.ios_device.is_none()));
    assert!(items.iter().all(EngineItem::available));
    assert_eq!(items[0].label, "Chromium");
    assert_eq!(items[1].label, "Firefox");
    // The text comes with the wave's locale part; until it is merged `t` answers the key.
    assert!(
        items[2].label.starts_with("WebKit") || items[2].label == WEBKIT_ROW,
        "{}",
        items[2].label
    );
}

#[test]
fn the_menu_names_webkit_by_playwright_beside_safari_and_the_badge_says_it_is_not_safari() {
    // RB-097-02: on a Mac both rows can be chosen; they must not read alike.
    let items = items(EngineKind::Chromium, |_| None);
    assert!(
        items[2].label == "WebKit (Playwright)" || items[2].label == WEBKIT_ROW,
        "{}",
        items[2].label
    );
    assert_eq!(items[3].label, "Safari");
    let webkit = badge(EngineKind::Webkit, None);
    assert_eq!(webkit.text, "WebKit");
    assert!(webkit.tooltip.contains("not Safari"), "{}", webkit.tooltip);
    let unavailable = items_with_webkit_reason();
    assert!(
        unavailable.label.contains("Playwright") || unavailable.label.contains(WEBKIT_ROW),
        "{}",
        unavailable.label
    );
}

fn items_with_webkit_reason() -> EngineItem {
    items(EngineKind::Chromium, |engine| {
        (engine == EngineKind::Webkit).then(|| "WebKit is not installed".to_owned())
    })
    .swap_remove(2)
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

#[test]
fn safari_outside_macos_is_a_disabled_row_with_its_reason() {
    let items = items(EngineKind::Chromium, |engine| {
        (engine == EngineKind::Safari).then(|| "Safari runs only on macOS".to_owned())
    });
    let safari = items
        .iter()
        .find(|item| item.engine == EngineKind::Safari)
        .expect("a Safari row");
    assert!(!safari.available());
    assert_eq!(safari.reason.as_deref(), Some("Safari runs only on macOS"));
    assert_eq!(command_args(safari, ViewId(3))["engine"], "safari");
}

#[test]
fn an_ios_device_row_writes_its_device_beside_the_engine() {
    let mut row = items(EngineKind::Chromium, |_| None)
        .pop()
        .expect("the iOS row");
    assert_eq!(row.engine, EngineKind::Ios);
    assert_eq!(
        command_args(&row, ViewId(3)),
        json!({ "view": 3, "engine": "ios" })
    );
    row.ios_device = Some("iPhone 16".to_owned());
    assert_eq!(
        command_args(&row, ViewId(3)),
        json!({ "view": 3, "engine": "ios", "iosDevice": "iPhone 16" })
    );
}
