use rb_config::DevtoolsMode;
use rb_engine::ViewId;

use super::super::session::tests::{attach_views, docks, open_prepared, setup, until};
use super::*;
use crate::devtools::layout::{MAX_DOCK, MIN_DOCK};

#[test]
fn the_page_reports_moves_and_the_end_of_a_drag_in_its_title() {
    assert_eq!(parse_title("rb-splitter:move:1:-40"), Some(Drag::Move(-40)));
    assert_eq!(parse_title("rb-splitter:move:2:15"), Some(Drag::Move(15)));
    assert_eq!(parse_title("rb-splitter:end:3:-60"), Some(Drag::End(-60)));
    for other in [
        "",
        "data:text/html,...",
        "rb-splitter:move:1",
        "rb-splitter:move:x:5",
        "rb-splitter:move:1:5px",
        "rb-splitter:drop:1:5",
        "rb-splitter:move:1:99999999999",
        "rb-splitter:move:1:200001",
    ] {
        assert_eq!(parse_title(other), None, "{other}");
    }
}

#[test]
fn dragging_towards_the_grid_grows_the_dock_within_its_bounds() {
    // Up (bottom) or left (right) is a negative delta: the dock grows.
    assert_eq!(dragged(320, -80), 400);
    assert_eq!(dragged(480, 100), 380);
    assert_eq!(dragged(200, 1_000), MIN_DOCK);
    assert_eq!(dragged(3_900, -1_000), MAX_DOCK);
}

#[test]
fn the_page_is_a_self_contained_data_url_that_names_its_own_axis() {
    let url = page_url();
    assert!(url.starts_with("data:text/html;charset=utf-8,"), "{url}");
    assert!(may_load(&url));
    assert!(!may_load("https://example.com/"));
    assert!(!may_load("data:text/html,<script>evil()</script>"));
    assert!(!may_load("javascript:alert(1)"));
    assert!(!may_load("rbrowser://ui/settings.html"));
    let body = &url["data:text/html;charset=utf-8,".len()..];
    // Nothing that would end or break the URL stays unescaped.
    for raw in ['#', ' ', '"', '<', '>', '\n', '\''] {
        assert!(!body.contains(raw), "{raw:?} in {body}");
    }
    let bytes = body.as_bytes();
    for (index, byte) in bytes.iter().enumerate() {
        if *byte == b'%' {
            assert!(
                bytes
                    .get(index + 1..index + 3)
                    .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit)),
                "a bare % at {index}"
            );
        }
    }
    let decoded = decode(body);
    assert!(decoded.contains("setPointerCapture"), "{decoded}");
    assert!(
        decoded.contains("screenY") && decoded.contains("screenX"),
        "{decoded}"
    );
}

/// Percent-decodes `text` (test helper).
fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).expect("ASCII");
            out.push(u8::from_str_radix(hex, 16).expect("hex"));
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).expect("UTF-8")
}

#[tokio::test]
async fn a_drag_resizes_the_dock_live_and_keeps_its_end_as_the_setting() {
    let setup = setup();
    attach_views(&setup).await;
    let opened = open_prepared(&setup).await;
    assert_eq!((opened.side, opened.size), (DockSide::Bottom, 320));
    on_title(&setup.core, "rb-splitter:move:1:-30");
    on_title(&setup.core, "rb-splitter:move:2:-80");
    let live: Vec<u32> = docks(&setup.ui)
        .into_iter()
        .flatten()
        .map(|dock| dock.size)
        .collect();
    assert_eq!(live[live.len() - 2..], [350, 400]);
    // Moving does not write the setting yet.
    assert_eq!(setup.core.user_settings().devtools.dock_height, 320);
    on_title(&setup.core, "rb-splitter:end:3:-100");
    assert_eq!(setup.core.user_settings().devtools.dock_height, 420);
    let last = docks(&setup.ui).last().cloned().flatten().expect("docked");
    assert_eq!(
        (last.side, last.size, last.view),
        (DockSide::Bottom, 420, ViewId(1))
    );
    let path = setup.core.user_settings_path().expect("a settings file");
    let kept = || {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
            .is_some_and(|file| file["devtools"]["dockHeight"] == 420)
    };
    until(kept).await;
    // The next drag starts from the new size.
    on_title(&setup.core, "rb-splitter:move:4:10");
    let last = docks(&setup.ui).last().cloned().flatten().expect("docked");
    assert_eq!(last.size, 410);
}

#[tokio::test]
async fn the_right_dock_keeps_its_width() {
    let setup = setup();
    attach_views(&setup).await;
    setup
        .devtools
        .open(ViewId(1), DevtoolsMode::DockedRight)
        .expect("opens");
    on_title(&setup.core, "rb-splitter:end:1:-20");
    let settings = setup.core.user_settings().devtools;
    assert_eq!((settings.dock_width, settings.dock_height), (500, 320));
}

#[tokio::test]
async fn without_a_dock_the_splitter_does_nothing() {
    let setup = setup();
    attach_views(&setup).await;
    let before = docks(&setup.ui).len();
    on_title(&setup.core, "rb-splitter:end:1:-100");
    assert_eq!(docks(&setup.ui).len(), before);
    assert_eq!(setup.core.user_settings().devtools.dock_height, 320);
}

#[tokio::test]
async fn a_changed_dock_size_setting_lays_the_open_dock_out_again() {
    let setup = setup();
    attach_views(&setup).await;
    open_prepared(&setup).await;
    let mut user = setup.core.user_settings();
    user.devtools.dock_height = 500;
    setup.core.replace_user_settings(user);
    setup.devtools.on_settings_changed().expect("moves");
    let last = docks(&setup.ui).last().cloned().flatten().expect("docked");
    assert_eq!(last.size, 500);
}
