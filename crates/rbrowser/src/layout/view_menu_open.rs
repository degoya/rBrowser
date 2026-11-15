//! The requests that open the native menus of the views (RB-080-46): the canvas board's
//! right-click (`canvasMenu`) and the toolbar's "+" (`addViewMenu`); and the status bar's zoom
//! presets (`canvasZoomMenu`, RB-090-18), whose `<select>` opened below the bar, off the screen
//! with the bar at its bottom. The board is a browser view
//! behind the views' overlays and the toolbar a strip 44 DIP tall, so a menu either page drew
//! itself lay behind the views or was cut off; they ask the core, and the core shows a native
//! menu in front of the views ([`MenuRequest`], `cef_app/view_menus.rs`). `ui.viewContextMenu`
//! off keeps the board without menus; the "+" stays.

use rb_commands::CommandError;
use rb_config::LayoutMode;
use rb_engine::ViewId;
use rb_ipc::{CoreToShell, IpcErrorCode, ShellToCore};

use super::canvas_board::{board_camera, board_point, checked, on_board};
use super::canvas_ops::{canvas_state, mode_and_area};
use super::view_menu::{MenuAnchor, MenuRequest};
use crate::control::Core;
use crate::ipc::Failure;

/// The menus' requests (`crate::features` through the board's); `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let done = match *message {
        ShellToCore::CanvasMenu { view, x, y } => board_menu(core, view, x, y),
        ShellToCore::AddViewMenu { x, y } => toolbar_menu(core, x, y),
        ShellToCore::CanvasZoomMenu { x, y } => zoom_menu(core, x, y),
        _ => return None,
    };
    if let Err(error) = &done {
        tracing::warn!(%error, "a view menu was not opened");
    }
    Some(
        done.map(|()| CoreToShell::Ack)
            .map_err(|error| (IpcErrorCode::BadRequest, error.to_string())),
    )
}

/// A right-click on the board at `x`/`y` (DIP from its corner): the menu of `view`, or the add
/// menu of the free area with the click as a canvas point. Nothing with `ui.viewContextMenu`
/// off.
fn board_menu(core: &Core, view: Option<ViewId>, x: i32, y: i32) -> Result<(), CommandError> {
    checked(x, y)?;
    let host = core.host();
    let (spec, area) = mode_and_area(host, LayoutMode::Canvas)?;
    if !core.user_settings().ui.view_context_menu {
        return Ok(());
    }
    let anchor = MenuAnchor::Board { x, y };
    let menu = match view {
        Some(view) => {
            if !on_board(host, &spec, view) {
                return Err(CommandError::InvalidArgs(format!(
                    "view {view} is not on the canvas"
                )));
            }
            MenuRequest::View {
                view,
                anchor,
                canvas: true,
            }
        }
        None => {
            let (_, camera) = canvas_state(&spec, &host.mode_sizes(), area);
            MenuRequest::Add {
                anchor,
                at: Some(board_point(board_camera(camera), x, y)),
            }
        }
    };
    tracing::info!(?menu, "the canvas board asked for a menu");
    host.ui().show_menu(menu);
    Ok(())
}

/// The toolbar's "+" at `x`/`y` (DIP from the shell view's corner): the add menu.
fn toolbar_menu(core: &Core, x: i32, y: i32) -> Result<(), CommandError> {
    checked(x, y)?;
    let menu = MenuRequest::Add {
        anchor: MenuAnchor::Toolbar { x, y },
        at: None,
    };
    tracing::info!(?menu, "the toolbar asked for the add menu");
    core.host().ui().show_menu(menu);
    Ok(())
}

/// The status bar's level button at `x`/`y` (DIP from the bar's corner): the zoom presets, only
/// on the canvas.
fn zoom_menu(core: &Core, x: i32, y: i32) -> Result<(), CommandError> {
    checked(x, y)?;
    let host = core.host();
    mode_and_area(host, LayoutMode::Canvas)?;
    let menu = MenuRequest::Zoom {
        anchor: MenuAnchor::StatusBar { x, y },
        level: super::canvas_level::current(host),
    };
    tracing::info!(?menu, "the status bar asked for the zoom presets");
    host.ui().show_menu(menu);
    Ok(())
}

#[cfg(test)]
#[path = "view_menu_open_tests.rs"]
mod tests;
