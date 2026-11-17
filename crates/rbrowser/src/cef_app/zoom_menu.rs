//! The canvas zoom's presets as a native menu above the status bar's level button (RB-090-18):
//! Fit, a separator, 25 to 200 %, the canvas's level checked ([`crate::layout::zoom_menu`]).
//! They were a `<select>` of the bar's page, whose list opened below the bar - off the screen
//! with the bar at the bottom of a fullscreen or maximised window. The menu hangs with its
//! bottom centre at the button's top edge (`MenuAnchorPosition::BOTTOMCENTER`), so it opens
//! above the bar; where no room is left above, the menu runner places it in the screen. UI
//! thread only.

use std::sync::Arc;

use cef::*;

use super::cell_labels::{id_of, row};
use super::views::core;
use crate::layout::zoom_menu::{self, ZoomAction, ZoomRow};

/// The command id of the first row (after the view menus', `view_menus.rs`).
const FIRST_ID: i32 = 27_000;

wrap_menu_model_delegate! {
    // The presets: row `FIRST_ID` + index.
    struct ZoomMenu {
        rows: Arc<Vec<ZoomRow>>,
    }

    impl MenuModelDelegate {
        fn execute_command(
            &self,
            _menu_model: Option<&mut MenuModel>,
            command_id: std::os::raw::c_int,
            _event_flags: EventFlags,
        ) {
            let Some(core) = core() else {
                return;
            };
            let Some(item) = row(command_id, FIRST_ID, &self.rows) else {
                return;
            };
            tracing::info!(action = ?item.action, "a zoom preset was chosen");
            zoom_menu::run(core, item.action);
        }
    }
}

/// Shows the presets in `window` with the menu's bottom centre at `point` (screen DIP), the
/// row at `level` checked.
pub fn show_at(window: &Window, point: Point, level: Option<f64>) {
    let rows = Arc::new(zoom_menu::rows(level));
    let mut delegate = ZoomMenu::new(rows.clone());
    let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
        tracing::warn!("the zoom presets could not be created");
        return;
    };
    for (index, item) in rows.iter().enumerate() {
        let Some(id) = id_of(FIRST_ID, index) else {
            continue;
        };
        model.add_check_item(id, Some(&CefString::from(item.label.as_str())));
        model.set_checked(id, i32::from(item.checked));
        if item.action == ZoomAction::Fit {
            model.add_separator();
        }
    }
    tracing::info!(
        x = point.x,
        y = point.y,
        ?level,
        "the zoom presets open above the status bar"
    );
    super::theme::paint_menu(&model);
    window.show_menu(
        Some(&mut model),
        Some(&point),
        MenuAnchorPosition::BOTTOMCENTER,
    );
}
