//! The toolbar's pulldowns as native menus (RB-090-41, [`crate::layout::native_menu`]): the
//! theme, the environment, the layout preset, the bookmarks and the overflow. The shell names
//! its button's rectangle and the rows; the menu hangs below the button towards the window's
//! middle (`MenuAnchorPosition::TOPLEFT` or `TOPRIGHT`), the menu runner keeps it on the
//! screen, and it is painted in rBrowser's theme like the zoom presets and the view menus
//! (`theme::paint_menu`). A heading is a row that cannot be chosen. The row chosen is told to
//! the shell once the menu closed ([`Pending`]). UI thread only.
//!
//! RB-090-63: the main menu `≡` and the split buttons bring submenus, radio rows and a
//! command's shortcut beside its row (`set_accelerator` for a direct one; a leader chord is in
//! the label, [`crate::layout::native_menu_entries`]).

use std::sync::Arc;

use cef::*;

use super::cell_labels::id_of;
use super::views::UI;
use crate::layout::native_menu::{self, Corner, Entry, NativeMenu, NativeMenuDone, Pending};

/// The radio group of a menu's choices: each submenu is a model of its own, and no menu of the
/// toolbar has two sets of choices in one level.
const RADIO_GROUP: i32 = 1;

/// The command id of the first row (after the zoom presets', `zoom_menu.rs`; up to
/// `native_menu::MAX_TOTAL` rows with every submenu's, below the page context menu's 27 900).
const FIRST_ID: i32 = 27_100;

wrap_menu_model_delegate! {
    // The rows: row `FIRST_ID` + index.
    struct ShellMenu {
        pending: Arc<Pending>,
    }

    impl MenuModelDelegate {
        fn execute_command(
            &self,
            _menu_model: Option<&mut MenuModel>,
            command_id: std::os::raw::c_int,
            _event_flags: EventFlags,
        ) {
            let index = command_id
                .checked_sub(FIRST_ID)
                .and_then(|index| usize::try_from(index).ok());
            if let Some(index) = index {
                self.pending.choose(index);
            }
        }

        fn menu_closed(&self, _menu_model: Option<&mut MenuModel>) {
            self.pending.close();
        }
    }
}

/// Shows `menu` below its button in the shell view; `done` hears the row chosen.
pub fn show(menu: NativeMenu, done: NativeMenuDone) {
    let pending = Arc::new(Pending::new(&menu.rows, done));
    let target = UI.with(|ui| {
        let ui = ui.borrow();
        let window = ui.window.clone()?;
        let bounds = View::from(ui.shell.as_ref()?).bounds_in_screen();
        Some((window, bounds))
    });
    let Some((window, bounds)) = target else {
        tracing::warn!(menu = %menu.menu, "a native menu has no window to open in");
        pending.close();
        return;
    };
    let mut delegate = ShellMenu::new(pending.clone());
    let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
        tracing::warn!(menu = %menu.menu, "a native menu could not be created");
        pending.close();
        return;
    };
    render(&mut model, native_menu::entries(&menu.rows));
    let place = native_menu::placement(menu.anchor, bounds.width);
    let point = Point {
        x: bounds.x.saturating_add(place.x),
        y: bounds.y.saturating_add(place.y),
    };
    let anchor = match place.corner {
        Corner::TopLeft => MenuAnchorPosition::TOPLEFT,
        Corner::TopRight => MenuAnchorPosition::TOPRIGHT,
    };
    tracing::info!(
        menu = %menu.menu,
        x = point.x,
        y = point.y,
        corner = ?place.corner,
        "a toolbar pulldown opens as a native menu"
    );
    super::theme::paint_menu(&model);
    window.show_menu(Some(&mut model), Some(&point), anchor);
}

/// Adds `entries` to `model`: rows with their mark and shortcut, headings, lines, submenus.
fn render(model: &mut MenuModel, entries: Vec<Entry>) {
    for entry in entries {
        match entry {
            Entry::Separator => {
                model.add_separator();
            }
            Entry::Heading { index, label } => {
                let Some(id) = id_of(FIRST_ID, index) else {
                    continue;
                };
                model.add_item(id, Some(&CefString::from(label.as_str())));
                model.set_enabled(id, 0);
            }
            Entry::Submenu {
                index,
                label,
                entries,
            } => {
                let Some(id) = id_of(FIRST_ID, index) else {
                    continue;
                };
                if let Some(mut submenu) =
                    model.add_sub_menu(id, Some(&CefString::from(label.as_str())))
                {
                    render(&mut submenu, entries);
                    // A submenu is a model of its own: the theme's colours are set on it too.
                    super::theme::paint_menu(&submenu);
                }
            }
            Entry::Item {
                index,
                label,
                checked,
                radio,
                accelerator,
                enabled,
            } => {
                let Some(id) = id_of(FIRST_ID, index) else {
                    continue;
                };
                let label = CefString::from(label.as_str());
                match (checked, radio) {
                    (Some(checked), true) => {
                        model.add_radio_item(id, Some(&label), RADIO_GROUP);
                        model.set_checked(id, i32::from(checked));
                    }
                    (Some(checked), false) => {
                        model.add_check_item(id, Some(&label));
                        model.set_checked(id, i32::from(checked));
                    }
                    (None, _) => {
                        model.add_item(id, Some(&label));
                    }
                }
                if let Some(keys) = accelerator {
                    model.set_accelerator(
                        id,
                        keys.key_code,
                        i32::from(keys.shift),
                        i32::from(keys.ctrl),
                        i32::from(keys.alt),
                    );
                }
                if !enabled {
                    model.set_enabled(id, 0);
                }
            }
        }
    }
}
