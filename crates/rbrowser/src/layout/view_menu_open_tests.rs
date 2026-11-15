use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::ShellToCore;
use serde_json::json;

use super::*;
use crate::host::ui::recording::UiCall;
use crate::layout::canvas::Camera;
use crate::layout::canvas_ops::set_camera;
use crate::layout::test_support::{Setup, config, setup};
use crate::layout::view_menu::{MenuAnchor, MenuRequest};

/// The canvas with the camera at (100, 50), zoom 0.5.
fn board() -> Setup {
    let setup = setup(config());
    crate::presets::run_command(
        &setup.core,
        "layout.select_preset",
        &json!({ "id": "board" }),
    )
    .expect("a presets command")
    .expect("selects");
    set_camera(
        &setup.core,
        Camera {
            x: 100.0,
            y: 50.0,
            zoom: 0.5,
        },
    );
    setup
}

fn menus(setup: &Setup) -> Vec<MenuRequest> {
    setup
        .ui
        .calls()
        .into_iter()
        .filter_map(|call| match call {
            UiCall::Menu(menu) => Some(menu),
            _ => None,
        })
        .collect()
}

/// The answer, or the refusal's words after `error:`.
fn send(core: &Core, message: &ShellToCore) -> String {
    let request = serde_json::to_string(message).expect("encodes");
    crate::ipc::dispatch(core, &request).unwrap_or_else(|(_, refusal)| format!("error: {refusal}"))
}

fn mobile(core: &Core) -> ViewId {
    core.host()
        .views()
        .into_iter()
        .find(|view| view.cell_breakpoint.as_deref() == Some("mobile"))
        .expect("mobile")
        .id
}

// RB-080-46: the board drew its menus itself, behind the views' native overlays. It asks the
// core now, and the core shows a native menu in front of them.
#[tokio::test]
async fn a_right_click_on_a_board_label_asks_for_the_native_view_menu_and_activates_nothing() {
    let setup = board();
    let core = &setup.core;
    let view = mobile(core);
    let active = core.host().active_view();
    assert_ne!(active, Some(view));
    let answer = send(
        core,
        &ShellToCore::CanvasMenu {
            view: Some(view),
            x: 120,
            y: 14,
        },
    );
    assert!(answer.contains("\"ack\""), "{answer}");
    assert_eq!(
        menus(&setup),
        [MenuRequest::View {
            view,
            anchor: MenuAnchor::Board { x: 120, y: 14 },
            canvas: true,
        }]
    );
    assert_eq!(core.host().active_view(), active);
    // A view that is not on the canvas has no menu.
    let answer = send(
        core,
        &ShellToCore::CanvasMenu {
            view: Some(ViewId(999)),
            x: 1,
            y: 1,
        },
    );
    assert!(answer.contains("error"), "{answer}");
    assert_eq!(menus(&setup).len(), 1);
}

#[tokio::test]
async fn a_right_click_on_the_free_board_asks_for_the_add_menu_at_the_canvas_point() {
    let setup = board();
    send(
        &setup.core,
        &ShellToCore::CanvasMenu {
            view: None,
            x: 300,
            y: 200,
        },
    );
    // Through the camera: 100 + 300 / 0.5, 50 + 200 / 0.5.
    assert_eq!(
        menus(&setup),
        [MenuRequest::Add {
            anchor: MenuAnchor::Board { x: 300, y: 200 },
            at: Some((700.0, 450.0)),
        }]
    );
}

#[tokio::test]
async fn the_board_opens_no_menu_with_the_setting_off_but_the_toolbar_plus_does() {
    let setup = board();
    let core = &setup.core;
    let mut user = core.user_settings();
    user.ui.view_context_menu = false;
    core.replace_user_settings(user);
    let view = mobile(core);
    for view in [Some(view), None] {
        let answer = send(core, &ShellToCore::CanvasMenu { view, x: 5, y: 5 });
        assert!(answer.contains("\"ack\""), "{answer}");
    }
    assert!(menus(&setup).is_empty());
    send(core, &ShellToCore::AddViewMenu { x: 96, y: 40 });
    assert_eq!(
        menus(&setup),
        [MenuRequest::Add {
            anchor: MenuAnchor::Toolbar { x: 96, y: 40 },
            at: None,
        }]
    );
}

// RB-090-18: the status bar's zoom presets were a `<select>` whose list opened below the bar,
// off the screen in fullscreen. The bar names the button's place and the core shows a native
// menu there, with the canvas's level for the check.
#[tokio::test]
async fn the_status_bar_asks_for_the_zoom_presets_at_its_button() {
    let setup = board();
    let core = &setup.core;
    let answer = send(core, &ShellToCore::CanvasZoomMenu { x: 420, y: 2 });
    assert!(answer.contains("\"ack\""), "{answer}");
    let level = crate::layout::canvas_level::current(core.host());
    assert!(level.is_some());
    assert_eq!(
        menus(&setup),
        [MenuRequest::Zoom {
            anchor: MenuAnchor::StatusBar { x: 420, y: 2 },
            level,
        }]
    );
    let answer = send(core, &ShellToCore::CanvasZoomMenu { x: 0, y: i32::MIN });
    assert!(answer.contains("error"), "{answer}");
    assert_eq!(menus(&setup).len(), 1);
}

#[tokio::test]
async fn outside_the_canvas_the_zoom_presets_do_not_open() {
    let setup = setup(config());
    let answer = send(&setup.core, &ShellToCore::CanvasZoomMenu { x: 420, y: 2 });
    assert!(answer.contains("error"), "{answer}");
    assert!(menus(&setup).is_empty());
}

#[tokio::test]
async fn a_point_far_outside_any_screen_is_refused() {
    let setup = board();
    let answer = send(&setup.core, &ShellToCore::AddViewMenu { x: i32::MAX, y: 0 });
    assert!(answer.contains("error"), "{answer}");
    assert!(menus(&setup).is_empty());
}
