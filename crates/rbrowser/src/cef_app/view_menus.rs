//! The native menus of the views that a page of the shell or a right-click asks for
//! (RB-080-46): the view menu ("Remove view", "Set as active", "Size...", "Pop out", on the
//! canvas "Center on this view", the engine submenu, then after a separator the footer switch
//! for every view, RB-080-60) and the add menu. A native menu lies in
//! front of the browser views in every layout mode; the menu the canvas board drew itself lay
//! behind them, and one the toolbar drew was cut off at its strip. The canvas board
//! (`canvasMenu`) and the toolbar's "+" (`addViewMenu`) name a point of their page
//! ([`MenuRequest`], [`show`]); a right-click on a label strip of the grid, the focus view, the
//! row or the split opens the view menu at the pointer (`strip_clicks.rs`,
//! [`show_view_menu_at`]). The rows come from [`crate::layout::view_menu`]; [`add_view_rows`]
//! draws them here and in the strip's size list. UI thread only.

use std::sync::Arc;

use cef::*;
use rb_engine::ViewId;

use super::cell_labels::{id_of, row, show_add_menu_at, show_quick_pick_at};
use super::task::on_ui;
use super::views::{UI, core};
use crate::breakpoint_keys::{self, EngineItem};
use crate::layout::view_menu::{self, MenuAnchor, MenuRequest, MenuRow, ViewAction, ViewMenuItem};

/// The first command id of the view menu's rows (after the add menu's, `cell_labels.rs`).
const VIEW_FIRST_ID: i32 = 26_900;
/// The engine submenu's id and the first id of its rows.
const ENGINE_MENU_ID: i32 = 26_919;
const ENGINE_FIRST_ID: i32 = 26_920;

wrap_menu_model_delegate! {
    // The menu of one view: its rows (`VIEW_FIRST_ID` + index) and the engine rows
    // (`ENGINE_FIRST_ID` + index).
    struct ViewMenu {
        view: ViewId,
        // Where the menu hangs, in screen DIP.
        x: i32,
        y: i32,
        items: Arc<Vec<ViewMenuItem>>,
        engines: Arc<Vec<EngineItem>>,
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
            if let Some(item) = row(command_id, ENGINE_FIRST_ID, &self.engines) {
                breakpoint_keys::pick_engine(core, self.view, item);
                return;
            }
            let Some(item) = row(command_id, VIEW_FIRST_ID, &self.items) else {
                return;
            };
            tracing::info!(view = %self.view, action = ?item.action, "a view menu row was chosen");
            if item.action == ViewAction::Size {
                // The sizes hang where this menu hung - after it has closed: the window runs
                // one menu at a time, and this one is still running here.
                let (view, x, y) = (self.view, self.x, self.y);
                on_ui(move || {
                    if let Some(window) = window_of(view) {
                        show_quick_pick_at(view, &window, Point { x, y });
                    }
                });
                return;
            }
            view_menu::run_view_action(core, self.view, item.action);
        }
    }
}

/// The window the strip of `view` is in; the main window for a view without one (the canvas
/// board draws its strips).
fn window_of(view: ViewId) -> Option<Window> {
    super::cell_labels::strip_of(view)
        .and_then(|strip| View::from(&strip).window())
        .or_else(|| UI.with(|ui| ui.borrow().window.clone()))
}

/// Shows the menu a page of the shell asked for.
pub fn show(menu: MenuRequest) {
    match menu {
        MenuRequest::View {
            view,
            anchor,
            canvas,
        } => {
            if let Some((window, point)) = resolve(anchor) {
                show_view_menu_at(view, &window, point, canvas);
            }
        }
        MenuRequest::Add { anchor, at } => {
            if let Some((window, point)) = resolve(anchor) {
                show_add_menu_at(&window, point, at);
            }
        }
        MenuRequest::Zoom { anchor, level } => {
            if let Some((window, point)) = resolve(anchor) {
                super::zoom_menu::show_at(&window, point, level);
            }
        }
    }
}

/// The window of `anchor`'s page and the screen point (DIP) it names.
fn resolve(anchor: MenuAnchor) -> Option<(Window, Point)> {
    let resolved = match anchor {
        MenuAnchor::Board { x, y } => super::canvas_board::screen_point(x, y),
        MenuAnchor::Toolbar { x, y } => UI.with(|ui| {
            let ui = ui.borrow();
            let window = ui.window.clone()?;
            let bounds = View::from(ui.shell.as_ref()?).bounds_in_screen();
            Some((
                window,
                Point {
                    x: bounds.x.saturating_add(x),
                    y: bounds.y.saturating_add(y),
                },
            ))
        }),
        MenuAnchor::StatusBar { x, y } => super::status_bar::screen_point(x, y),
    };
    if resolved.is_none() {
        tracing::warn!(?anchor, "a view menu has no window to open in");
    }
    resolved
}

/// Shows the view menu of `view` in `window` with its top-left corner at `point` (screen DIP);
/// on the canvas with "Center on this view". The view is not made active.
pub fn show_view_menu_at(view: ViewId, window: &Window, point: Point, canvas: bool) {
    let Some(core) = core() else {
        return;
    };
    let items = Arc::new(view_menu::menu_items(core, view, canvas));
    let engines: Arc<Vec<EngineItem>> =
        Arc::new(breakpoint_keys::engine_pick(core, view).unwrap_or_default());
    let mut delegate = ViewMenu::new(view, point.x, point.y, items.clone(), engines.clone());
    let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
        tracing::warn!(%view, "the view menu could not be created");
        return;
    };
    let ids = ViewRowIds {
        first: VIEW_FIRST_ID,
        engine_menu: ENGINE_MENU_ID,
        engine_first: ENGINE_FIRST_ID,
        engine_key: view_menu::KEY_ENGINE_PICK,
    };
    add_view_rows(&mut model, &items, &engines, &ids);
    tracing::info!(%view, x = point.x, y = point.y, canvas, rows = items.len(), "the view menu opens");
    super::theme::paint_menu(&model);
    window.show_menu(Some(&mut model), Some(&point), MenuAnchorPosition::TOPLEFT);
}

/// The command ids and the engine submenu's title of one menu's view rows.
pub(super) struct ViewRowIds {
    /// The id of the first view row; a row's id is this plus its index.
    pub first: i32,
    /// The engine submenu's id.
    pub engine_menu: i32,
    /// The id of the first engine row.
    pub engine_first: i32,
    /// The engine submenu's title key.
    pub engine_key: &'static str,
}

/// Adds a view's rows to `model` in the order of [`view_menu::layout_rows`]: its own rows, the
/// engine submenu, then - set apart - the footer switch for every view, a check row (RB-080-60).
/// One renderer for the right-click menu and the strip's size list.
pub(super) fn add_view_rows(
    model: &mut MenuModel,
    items: &[ViewMenuItem],
    engines: &[EngineItem],
    ids: &ViewRowIds,
) {
    for row in view_menu::layout_rows(items, !engines.is_empty()) {
        match row {
            MenuRow::Separator => {
                model.add_separator();
            }
            MenuRow::Item(index) => {
                let (Some(item), Some(id)) = (items.get(index), id_of(ids.first, index)) else {
                    continue;
                };
                let label = CefString::from(item.label.as_str());
                match item.checked {
                    Some(checked) => {
                        model.add_check_item(id, Some(&label));
                        model.set_checked(id, i32::from(checked));
                    }
                    None => {
                        model.add_item(id, Some(&label));
                    }
                }
            }
            MenuRow::Engines => {
                let title = CefString::from(rb_i18n::t(ids.engine_key).as_str());
                let Some(submenu) = model.add_sub_menu(ids.engine_menu, Some(&title)) else {
                    continue;
                };
                for (index, item) in engines.iter().enumerate() {
                    let Some(id) = id_of(ids.engine_first, index) else {
                        break;
                    };
                    submenu.add_check_item(id, Some(&CefString::from(item.label.as_str())));
                    submenu.set_checked(id, i32::from(item.checked));
                    submenu.set_enabled(id, i32::from(item.available()));
                }
            }
        }
    }
}
