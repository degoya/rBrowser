use rb_config::{CellAddress, CellLabelSettings};

use super::*;
use crate::cell_info::{CellInfo, Piece, PieceAction};

const BASE: &str = "http://localhost:5173";

#[test]
fn an_address_is_completed_as_the_address_bar_completes_it() {
    for (typed, url) in [
        ("localhost:5173/about", "http://localhost:5173/about"),
        ("  shop.test/cart  ", "http://shop.test/cart"),
        ("example.com", "https://example.com/"),
        ("https://example.com/a?b#c", "https://example.com/a?b#c"),
        ("HTTP://Example.com/A", "http://example.com/A"),
        ("file:///C:/site/index.html", "file:///C:/site/index.html"),
    ] {
        assert_eq!(resolve(typed, Some(BASE)), Ok(url.to_owned()), "{typed}");
    }
}

#[test]
fn a_path_without_a_host_opens_under_the_environment_in_force() {
    assert_eq!(
        resolve("/pricing?plan=pro#top", Some(BASE)),
        Ok("http://localhost:5173/pricing?plan=pro#top".to_owned())
    );
    assert_eq!(
        resolve("/", Some("https://staging.example.com/app/")),
        Ok("https://staging.example.com/app/".to_owned())
    );
    assert_eq!(resolve("/pricing", None), Err(Refusal::NoBase));
}

// RB-092-03 acceptance: script, data and browser-internal addresses never load, and the refusal
// names the scheme.
#[test]
fn only_http_https_and_file_are_opened() {
    for (typed, scheme) in [
        ("javascript:alert(1)", "javascript"),
        ("JavaScript:alert(1)", "javascript"),
        ("data:text/html,<b>hi</b>", "data"),
        ("chrome://settings", "chrome"),
        ("about:blank", "about"),
        ("view-source:http://localhost:5173/", "view-source"),
        ("rbrowser://ui/index.html", "rbrowser"),
    ] {
        assert_eq!(
            resolve(typed, Some(BASE)),
            Err(Refusal::Scheme(scheme.to_owned())),
            "{typed}"
        );
    }
}

#[test]
fn nothing_typed_and_junk_are_refused() {
    assert_eq!(resolve("   ", Some(BASE)), Err(Refusal::Empty));
    for junk in ["two words", "http://", "?query", "https://exa mple.com"] {
        assert_eq!(resolve(junk, Some(BASE)), Err(Refusal::Invalid), "{junk}");
    }
}

#[test]
fn every_refusal_has_its_words_and_the_reason() {
    let refusals = [
        Refusal::Off,
        Refusal::Coupled,
        Refusal::Empty,
        Refusal::NoBase,
        Refusal::Scheme("javascript".to_owned()),
        Refusal::Invalid,
    ];
    for refusal in &refusals {
        assert!(
            refusal.key().starts_with("cells.address.refused."),
            "{refusal:?}"
        );
        let message = refusal.message("Tablet");
        assert!(!message.contains('{'), "{message}");
        assert!(message.contains("Tablet"), "{message}");
    }
    assert!(
        Refusal::Scheme("javascript".to_owned())
            .message("Tablet")
            .contains("javascript:")
    );
    assert_eq!(Refusal::Off.settings_key(), Some(SETTING));
    assert_eq!(Refusal::Coupled.settings_key(), None);
}

#[test]
fn a_decoupled_view_may_edit_its_address_a_coupled_one_not() {
    let on = CellLabelSettings::default();
    assert_eq!(may_edit(&on, false), Ok(()));
    assert_eq!(may_edit(&on, true), Err(Refusal::Coupled));
    let off = CellLabelSettings {
        address_editable: false,
        ..CellLabelSettings::default()
    };
    assert_eq!(may_edit(&off, false), Err(Refusal::Off));
}

fn info(view: u32, address: bool) -> CellInfo {
    CellInfo {
        view: ViewId(view),
        address: address.then(|| Piece {
            text: "localhost:5173/a".to_owned(),
            tooltip: "Address of Tablet\nhttp://localhost:5173/a".to_owned(),
            action: PieceAction::Activate,
        }),
        engine: None,
        engine_in_badge: false,
    }
}

#[test]
fn the_address_of_a_decoupled_view_opens_the_field_on_a_click() {
    let settings = CellLabelSettings {
        address: CellAddress::Always,
        ..CellLabelSettings::default()
    };
    let mut infos = vec![info(1, true), info(2, true), info(3, false)];
    mark_editable(&mut infos, &[ViewId(2), ViewId(3)], &settings);
    let first = infos[0].address.as_ref().expect("view 1's address");
    assert_eq!(first.action, PieceAction::Activate, "a coupled view");
    let second = infos[1].address.as_ref().expect("view 2's address");
    assert_eq!(second.action, PieceAction::EditAddress);
    assert!(
        second
            .tooltip
            .starts_with("Address of Tablet\nhttp://localhost:5173/a\n"),
        "{}",
        second.tooltip
    );
    assert_eq!(second.tooltip.lines().count(), 3, "{}", second.tooltip);
    assert_eq!(infos[2].address, None, "no piece, nothing to mark");

    let off = CellLabelSettings {
        address_editable: false,
        ..settings
    };
    let mut infos = vec![info(2, true)];
    mark_editable(&mut infos, &[ViewId(2)], &off);
    assert_eq!(
        infos[0],
        info(2, true),
        "off: the click activates as before"
    );
}

#[test]
fn the_field_starts_empty_on_a_view_that_loaded_nothing_yet() {
    assert_eq!(field_text("about:blank#rb-view-2"), "");
    assert_eq!(
        field_text("http://localhost:5173/a"),
        "http://localhost:5173/a"
    );
}

#[test]
fn enter_loads_escape_cancels_and_every_other_key_types() {
    assert_eq!(field_key(0x0D, true), Some(FieldKey::Submit));
    assert_eq!(field_key(0x1B, true), Some(FieldKey::Cancel));
    assert_eq!(
        field_key(0x0D, false),
        None,
        "the key's release does nothing"
    );
    assert_eq!(field_key(0x41, true), None);
}
