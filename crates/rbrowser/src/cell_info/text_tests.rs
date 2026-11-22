use rb_config::EngineKind;
use serde_json::json;

use super::*;

fn view(id: u32, url: &str) -> ViewInfo {
    ViewInfo {
        id: ViewId(id),
        label: format!("view {id}"),
        breakpoint: None,
        cell_breakpoint: None,
        engine: EngineKind::Chromium,
        width: 390,
        height: 844,
        dpr: 1.0,
        scale: 1.0,
        url: url.to_owned(),
        title: String::new(),
        loading: false,
        session: "default".to_owned(),
        cell: None,
    }
}

fn external(value: serde_json::Value) -> ExternalView {
    let mut base = json!({
        "view": 2, "label": "Tablet", "breakpoint": "tablet", "engine": "firefox",
        "status": "running", "url": "http://localhost:3000/"
    });
    if let (Some(base), Some(extra)) = (base.as_object_mut(), value.as_object()) {
        base.extend(extra.clone());
    }
    serde_json::from_value(base).expect("external view")
}

#[test]
fn a_stopped_engine_says_so_and_its_click_restarts_it() {
    let piece = engine_piece(&external(json!({ "status": "exited" })), false);
    assert_eq!(piece.text, "Firefox: engine stopped");
    assert_eq!(piece.action, PieceAction::Restart);
    assert!(piece.tooltip.starts_with("The Firefox engine stopped."));
    assert!(
        piece.tooltip.ends_with("Click: Restart"),
        "{}",
        piece.tooltip
    );
}

#[test]
fn a_closed_window_opens_again_and_a_window_of_its_own_comes_to_the_front() {
    let closed = engine_piece(&external(json!({ "status": "closed" })), false);
    assert_eq!(closed.action, PieceAction::Restart);
    assert!(closed.tooltip.ends_with("Click: Open again"));
    let own = engine_piece(&external(json!({ "status": "running" })), false);
    assert_eq!(own.text, "Firefox: external window");
    assert_eq!(own.action, PieceAction::Raise);
    assert!(own.tooltip.ends_with("Click: Show window"));
}

#[test]
fn a_window_in_its_cell_says_so_and_its_click_only_activates() {
    let piece = engine_piece(
        &external(json!({ "status": "running", "embedded": true })),
        false,
    );
    assert_eq!(piece.text, "Firefox: in its cell");
    assert_eq!(piece.action, PieceAction::Activate);
    assert!(!piece.tooltip.contains("Click:"));
}

#[test]
fn what_the_engine_does_not_emulate_is_in_the_tooltip() {
    let piece = engine_piece(
        &external(json!({
            "status": "running", "embedded": true, "unsupported": ["touch", "dpr"]
        })),
        false,
    );
    assert_eq!(piece.tooltip, "Not emulated in Firefox: touch, dpr");
}

#[test]
fn an_unavailable_engine_names_the_reason() {
    let piece = engine_piece(
        &external(json!({
            "engine": "webkit", "status": "unavailable", "reason": "Playwright is not installed"
        })),
        false,
    );
    assert!(
        piece.text.ends_with(": not available yet"),
        "{}",
        piece.text
    );
    assert!(
        piece.tooltip.starts_with("Playwright is not installed."),
        "{}",
        piece.tooltip
    );
}

#[test]
fn the_address_shows_by_default_only_where_it_differs_from_the_active_view() {
    let leader = view(1, "http://localhost:3000/a");
    let same = view(2, "http://localhost:3000/a");
    let other = view(3, "http://localhost:3000/b");
    let differs = CellAddress::Differs;
    assert_eq!(address_piece(&leader, Some(&leader), differs), None);
    assert_eq!(address_piece(&same, Some(&leader), differs), None);
    let piece = address_piece(&other, Some(&leader), differs).expect("differs");
    assert_eq!(piece.text, "localhost:3000/b");
    assert_eq!(piece.tooltip, "Address of view 3\nhttp://localhost:3000/b");
    assert_eq!(
        address_piece(&other, None, differs),
        None,
        "no leader, nothing to differ from"
    );
}

#[test]
fn always_shows_every_address_and_off_none() {
    let leader = view(1, "https://example.com/");
    assert!(address_piece(&leader, Some(&leader), CellAddress::Always).is_some());
    assert_eq!(
        address_piece(&leader, Some(&leader), CellAddress::Off),
        None
    );
    assert_eq!(
        address_piece(&view(2, ""), None, CellAddress::Always),
        None,
        "no address yet"
    );
}

#[test]
fn the_strip_writes_an_address_without_its_scheme_and_a_bare_origin_without_its_slash() {
    assert_eq!(display_url("https://example.com/"), "example.com");
    assert_eq!(display_url("http://localhost:3000/"), "localhost:3000");
    assert_eq!(
        display_url("http://localhost:3000/shop/?q=1#top"),
        "localhost:3000/shop/?q=1#top"
    );
    assert_eq!(display_url("about:blank"), "about:blank");
    assert_eq!(
        display_url("file:///C:/site/index.html"),
        "file:///C:/site/index.html"
    );
}

#[test]
fn each_setting_switches_its_piece() {
    let views = [
        view(1, "http://localhost:3000/"),
        view(2, "http://localhost:3000/b"),
    ];
    let externals = [external(json!({ "status": "starting" }))];
    let all = CellLabelSettings::default();
    let infos = cell_infos(&views, Some(ViewId(1)), &externals, &all, true);
    assert_eq!(infos.len(), 2);
    assert_eq!(infos[0].view, ViewId(1));
    assert_eq!((&infos[0].address, &infos[0].engine), (&None, &None));
    assert!(infos[1].address.is_some() && infos[1].engine.is_some());
    let none = CellLabelSettings {
        engine_status: false,
        address: CellAddress::Off,
        ..CellLabelSettings::default()
    };
    let infos = cell_infos(&views, Some(ViewId(1)), &externals, &none, true);
    assert_eq!((&infos[1].address, &infos[1].engine), (&None, &None));
}

// RB-090-71: the engine's state stands in the engine badge beside its name - one badge,
// `Firefox \u{b7} window closed` - while the badge shows; without the badge it is a piece of
// its own.
#[test]
fn the_engine_state_merges_into_the_engine_badge() {
    let views = [view(2, "http://localhost:3000/")];
    let externals = [external(json!({ "status": "closed" }))];
    let settings = CellLabelSettings::default();
    let merged = cell_infos(&views, None, &externals, &settings, true);
    let piece = merged[0].engine.as_ref().expect("the state");
    assert!(merged[0].engine_in_badge);
    assert_eq!(piece.text, "Firefox \u{b7} window closed");
    assert_eq!(piece.action, PieceAction::Restart);
    let apart = cell_infos(&views, None, &externals, &settings, false);
    assert!(!apart[0].engine_in_badge);
    assert_eq!(
        apart[0].engine.as_ref().map(|piece| piece.text.as_str()),
        Some("Firefox: window closed")
    );
}

#[test]
fn the_badge_menu_offers_what_a_click_on_the_state_did() {
    let closed = external(json!({ "status": "closed" }));
    assert_eq!(
        engine_click(&closed),
        Some((PieceAction::Restart, "engines.external.reopen"))
    );
    let stopped = external(json!({ "status": "exited" }));
    assert_eq!(
        engine_click(&stopped),
        Some((PieceAction::Restart, "engines.external.restart"))
    );
    let failed = external(json!({ "status": "failed" }));
    assert_eq!(
        engine_click(&failed),
        Some((PieceAction::Restart, "engines.external.retry"))
    );
    let own = external(json!({ "status": "running" }));
    assert_eq!(
        engine_click(&own),
        Some((PieceAction::Raise, "engines.external.show"))
    );
    let in_cell = external(json!({ "status": "running", "embedded": true }));
    assert_eq!(engine_click(&in_cell), None);
}

#[test]
fn the_badge_menu_offers_the_state_only_while_the_label_shows_states() {
    let externals = [external(json!({ "status": "closed" }))];
    let on = CellLabelSettings::default();
    assert_eq!(
        engine_menu_row(&externals, ViewId(2), &on),
        Some((PieceAction::Restart, "engines.external.reopen"))
    );
    let off = CellLabelSettings {
        engine_status: false,
        ..CellLabelSettings::default()
    };
    assert_eq!(engine_menu_row(&externals, ViewId(2), &off), None);
    assert_eq!(
        engine_menu_row(&externals, ViewId(9), &on),
        None,
        "no such view"
    );
}

#[test]
fn a_dimmed_badge_names_its_reason_and_a_running_one_the_state_first() {
    let state = Piece {
        text: "Firefox \u{b7} window closed".to_owned(),
        tooltip: "The Firefox window was closed.".to_owned(),
        action: PieceAction::Restart,
    };
    let own = (
        "Firefox",
        "Firefox. Click to choose the engine of this view.",
    );
    assert_eq!(
        badge_words(own, false, Some(&state)),
        (
            "Firefox \u{b7} window closed".to_owned(),
            "The Firefox window was closed.\nFirefox. Click to choose the engine of this view."
                .to_owned()
        )
    );
    // An engine that cannot run: its reason is the badge's own tooltip, said once.
    let reason = ("Firefox", "Firefox cannot run: not found.");
    assert_eq!(
        badge_words(reason, true, Some(&state)),
        (
            "Firefox \u{b7} window closed".to_owned(),
            "Firefox cannot run: not found.".to_owned()
        )
    );
    assert_eq!(
        badge_words(own, false, None),
        (own.0.to_owned(), own.1.to_owned())
    );
}

// RB-094-20: a view that failed to start names its reason in the strip - cut, the whole of it
// in the tooltip - and its click (and the badge menu's row) is Retry, the engine's restart.
#[test]
fn a_failed_start_names_its_reason_and_retries() {
    let reason = "Chrome on the Android device R5CX does not answer. Open Chrome on the device \
                  once, keep it unlocked, then try again.";
    let failed = external(json!({ "engine": "android", "status": "failed", "reason": reason }));
    let piece = engine_piece(&failed, true);
    let status = rb_i18n::t_with(
        "engines.external.status.failedBecause",
        &[("reason", &rb_util::text::cut_ellipsis(reason, MAX_REASON))],
    );
    assert!(piece.text.ends_with(&status), "{}", piece.text);
    assert!(
        piece.text.contains("Chrome on the Android"),
        "{}",
        piece.text
    );
    assert!(piece.text.ends_with("..."), "cut: {}", piece.text);
    assert!(piece.tooltip.contains(reason), "{}", piece.tooltip);
    assert!(piece.tooltip.ends_with("Click: Retry"), "{}", piece.tooltip);
    assert_eq!(piece.action, PieceAction::Restart);
    // Without a reason the state alone.
    let bare = engine_piece(&external(json!({ "status": "failed" })), false);
    assert_eq!(bare.text, "Firefox: failed to start");
}

#[test]
fn on_macos_a_window_of_its_own_says_why_it_is_not_in_its_cell() {
    // RB-097-02: macOS lets no app take another app's window into its cell.
    assert_eq!(hint_id(ExternalStatus::Running, true), "ownWindowMacos");
    assert_eq!(hint_id(ExternalStatus::Running, false), "running");
    assert_eq!(hint_id(ExternalStatus::Closed, true), "closed");
}
