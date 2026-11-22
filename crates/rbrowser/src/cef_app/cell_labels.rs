//! The label strips of the grid's cells (RB-010-16): the label after a breakpoint shortcut,
//! first the overlay naming the new size for `breakpointShortcuts.overlayMs`, and the size quick
//! pick, a native menu under the strip of the active view (the canvas board opens the same menu
//! under the strip it draws, RB-080-22). Its rows come from
//! [`crate::breakpoint_keys::quick_pick`]; a choice runs `view.set_size`, and the new-view
//! modifier held while choosing opens a new view instead.
//!
//! With `ui.viewContextMenu` (RB-080-41) the same menu carries the view's rows below the sizes -
//! "Remove view", "Set as active", "Pop out" / "Back into the grid", the engine submenu and, set
//! apart, the footer switch for every view (RB-080-60; [`crate::layout::view_menu`],
//! [`crate::breakpoint_keys::engine_pick`], drawn by `view_menus::add_view_rows`): CEF's views report
//! a button's press without its mouse button, so the strip has no right-click of its own and
//! the active view's click is where its menu hangs. The add menu ([`show_add_menu_at`]) lists
//! the hidden breakpoints under the label strip of an empty cell ([`hidden_label`]). A
//! right-click on any strip opens its menu too (RB-080-46): the window hears the button
//! (`strip_clicks.rs`) and finds the strip among [`strips`]. UI thread only.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use cef::*;
use rb_commands::keymap::modifiers_from_event_flags;
use rb_engine::ViewId;

use super::task::on_ui_after;
use super::views::core;
use crate::breakpoint_keys::{self, EngineItem, PickItem, Section};
use crate::host::ui::CellOverlay;
use crate::layout::view_menu::{self, AddAction, AddMenuItem, Strip, ViewMenuItem};
use crate::page_window::ScreenRect;

/// The first command id of the quick pick's rows (CEF's `MENU_ID_USER_FIRST`).
const FIRST_ID: i32 = 26_500;
/// The command id of the device presets' submenu (after every row id).
const DEVICES_MENU_ID: i32 = FIRST_ID - 1;
/// The title of the device presets' submenu.
const DEVICES_KEY: &str = "presets.devices.label";
/// The first command id of the view rows below the sizes (RB-080-41).
const VIEW_FIRST_ID: i32 = 26_700;
/// The engine submenu's id and the first id of its rows.
const ENGINE_MENU_ID: i32 = 26_719;
const ENGINE_FIRST_ID: i32 = 26_720;
/// The first command id of the add menu's rows.
const ADD_FIRST_ID: i32 = 26_800;

thread_local! {
    /// Each cell's label strip and the number of its last change, so a late overlay restore
    /// never overwrites a newer label.
    static LABELS: RefCell<HashMap<ViewId, (LabelButton, u64)>> = RefCell::new(HashMap::new());
    /// The label strips of the empty cells, while they are in a window.
    static HIDDEN: RefCell<Vec<LabelButton>> = const { RefCell::new(Vec::new()) };
}

/// Keeps the label strip `label` of `view` (the grid built it).
pub fn remember(view: ViewId, label: &LabelButton) {
    LABELS.with(|labels| {
        let mut labels = labels.borrow_mut();
        let generation = labels.get(&view).map_or(0, |(_, generation)| *generation);
        labels.insert(view, (label.clone(), generation));
    });
    // The leader's strip in the accent colour (RB-080-04).
    super::active_label::style(view, label);
}

/// The label strip of `view`, when the grid built one.
pub fn strip_of(view: ViewId) -> Option<LabelButton> {
    LABELS.with(|labels| labels.borrow().get(&view).map(|(button, _)| button.clone()))
}

/// Every label strip that is drawn in a window - the views' and the empty cells' - with its
/// rectangle in screen DIP and its window: what a right-click can hit (RB-080-46). A strip the
/// grid built before and took out again (the canvas board draws its own) is in no window.
#[cfg_attr(
    any(target_os = "macos", target_os = "linux"),
    allow(
        dead_code,
        reason = "used by the Windows-only strip right-click; the macOS and Linux twins are no-ops"
    )
)]
pub fn strips() -> Vec<(Strip, ScreenRect, Window)> {
    let placed = |strip: Strip, button: &LabelButton| {
        let view = View::from(button);
        let window = view.window()?;
        if view.is_drawn() == 0 {
            return None;
        }
        let bounds = view.bounds_in_screen();
        let rect = ScreenRect {
            x: bounds.x,
            y: bounds.y,
            width: bounds.width,
            height: bounds.height,
        };
        Some((strip, rect, window))
    };
    let mut strips: Vec<(Strip, ScreenRect, Window)> = LABELS.with(|labels| {
        labels
            .borrow()
            .iter()
            .filter_map(|(view, (button, _))| placed(Strip::View(*view), button))
            .collect()
    });
    HIDDEN.with(|hidden| {
        strips.extend(
            hidden
                .borrow()
                .iter()
                .filter_map(|button| placed(Strip::Hidden, button)),
        );
    });
    strips
}

/// Runs `action` on each label strip the grid built.
pub fn each(mut action: impl FnMut(ViewId, &LabelButton)) {
    LABELS.with(|labels| {
        for (view, (button, _)) in labels.borrow().iter() {
            action(*view, button);
        }
    });
}

/// Writes `label` into the strip of `view`; with `overlay`, its text first, for its time.
pub fn set_label(view: ViewId, label: String, overlay: Option<CellOverlay>) {
    let shown = overlay
        .as_ref()
        .map_or(label.as_str(), |overlay| overlay.text.as_str())
        .to_owned();
    let generation = LABELS.with(|labels| {
        let mut labels = labels.borrow_mut();
        let (button, generation) = labels.get_mut(&view)?;
        *generation += 1;
        button.set_text(Some(&CefString::from(shown.as_str())));
        // The leader's sign before the new text (RB-090-22).
        super::active_label::style(view, button);
        Some(*generation)
    });
    let (Some(generation), Some(overlay)) = (generation, overlay) else {
        return;
    };
    on_ui_after(overlay.ms, move || {
        LABELS.with(|labels| {
            if let Some((button, current)) = labels.borrow().get(&view)
                && *current == generation
            {
                button.set_text(Some(&CefString::from(label.as_str())));
                super::active_label::style(view, button);
            }
        });
    });
}

wrap_menu_model_delegate! {
    // The menu of one view: the quick pick's rows (`FIRST_ID` + index), the view rows
    // (`VIEW_FIRST_ID` + index) and the engine rows (`ENGINE_FIRST_ID` + index).
    struct SizeMenu {
        view: ViewId,
        items: Arc<Vec<PickItem>>,
        view_items: Arc<Vec<ViewMenuItem>>,
        engines: Arc<Vec<EngineItem>>,
    }

    impl MenuModelDelegate {
        fn execute_command(
            &self,
            _menu_model: Option<&mut MenuModel>,
            command_id: std::os::raw::c_int,
            event_flags: EventFlags,
        ) {
            let Some(core) = core() else {
                return;
            };
            if let Some(item) = row(command_id, ENGINE_FIRST_ID, &self.engines) {
                breakpoint_keys::pick_engine(core, self.view, item);
                return;
            }
            if let Some(item) = row(command_id, VIEW_FIRST_ID, &self.view_items) {
                view_menu::run_view_action(core, self.view, item.action);
                return;
            }
            let Some(item) = row(command_id, FIRST_ID, &self.items) else {
                return;
            };
            let flags: cef::sys::cef_event_flags_t = event_flags.into();
            // The bits as they are: `cef_event_flags_t` is i32 on Windows and u32 on macOS.
            let modifiers = modifiers_from_event_flags(u32::from_ne_bytes(flags.0.to_ne_bytes()));
            breakpoint_keys::pick(core, self.view, item, modifiers);
        }
    }
}

wrap_menu_model_delegate! {
    // The add menu: a row's command id is `ADD_FIRST_ID` + its index; `at` is the canvas
    // point of a right-click on the free board.
    struct AddMenu {
        items: Arc<Vec<AddMenuItem>>,
        at: Option<(f64, f64)>,
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
            if let Some(action) = row(command_id, ADD_FIRST_ID, &self.items)
                .and_then(|item| item.action.as_ref())
            {
                view_menu::run_add_action(core, action, self.at);
            }
        }
    }
}

/// The row of `items` whose command id is `first` + its index.
pub(super) fn row<T>(command_id: i32, first: i32, items: &[T]) -> Option<&T> {
    command_id
        .checked_sub(first)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| items.get(index))
}

/// The command id of row `index` after `first`; `None` past `i32`.
pub(super) fn id_of(first: i32, index: usize) -> Option<i32> {
    i32::try_from(index)
        .ok()
        .and_then(|index| first.checked_add(index))
}

/// Shows the size quick pick of `view` under its label strip `button`.
pub fn show_quick_pick(view: ViewId, button: &Button) {
    let bounds = button.bounds_in_screen();
    let point = Point {
        x: bounds.x,
        y: bounds.y.saturating_add(bounds.height),
    };
    match button.window() {
        Some(window) => show_quick_pick_at(view, &window, point),
        None => tracing::warn!(%view, "the clicked label strip is in no window: no size list"),
    }
}

/// Shows the size quick pick of `view` in `window` with its top-left corner at `point` (screen
/// DIP): under a grid strip, or under the strip the canvas board draws (RB-080-22). With
/// `ui.viewContextMenu` the view's rows follow the sizes (RB-080-41).
pub fn show_quick_pick_at(view: ViewId, window: &Window, point: Point) {
    let Some(core) = core() else {
        return;
    };
    let items = Arc::new(breakpoint_keys::quick_pick(core, view));
    let with_view_rows = core.user_settings().ui.view_context_menu;
    // The right-click menu's rows, the footer switch included, without "Size...".
    let view_items: Arc<Vec<ViewMenuItem>> = Arc::new(view_menu::size_list_items(core, view));
    let engines: Arc<Vec<EngineItem>> = Arc::new(if with_view_rows {
        breakpoint_keys::engine_pick(core, view).unwrap_or_default()
    } else {
        Vec::new()
    });
    let mut delegate = SizeMenu::new(view, items.clone(), view_items.clone(), engines.clone());
    let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
        return;
    };
    let mut devices: Option<MenuModel> = None;
    let mut last: Option<Section> = None;
    for (index, item) in items.iter().enumerate() {
        let Some(id) = id_of(FIRST_ID, index) else {
            break;
        };
        if last.is_some_and(|last| last != item.section) {
            model.add_separator();
        }
        let menu = if item.section == Section::Devices {
            if devices.is_none() {
                devices = model.add_sub_menu(
                    DEVICES_MENU_ID,
                    Some(&CefString::from(rb_i18n::t(DEVICES_KEY).as_str())),
                );
            }
            devices.as_ref().unwrap_or(&model)
        } else {
            &model
        };
        menu.add_check_item(id, Some(&CefString::from(item.label.as_str())));
        menu.set_checked(id, i32::from(item.checked));
        last = Some(item.section);
    }
    if !view_items.is_empty() {
        model.add_separator();
        let ids = super::view_menus::ViewRowIds {
            first: VIEW_FIRST_ID,
            engine_menu: ENGINE_MENU_ID,
            engine_first: ENGINE_FIRST_ID,
            engine_key: view_menu::KEY_ENGINE,
        };
        super::view_menus::add_view_rows(&mut model, &view_items, &engines, &ids);
    }
    tracing::info!(%view, x = point.x, y = point.y, rows = items.len(), "the size list opens");
    super::theme::paint_menu(&model);
    window.show_menu(Some(&mut model), Some(&point), MenuAnchorPosition::TOPLEFT);
}

/// Shows the add menu (RB-080-41) in `window` with its top-left corner at `point` (screen DIP):
/// one row per hidden breakpoint, "Add all", "New size..."; "All breakpoints are open" when
/// none is hidden. With `at`, the canvas point of a right-click on the free board (RB-080-46),
/// a view is added there and the canvas's "Zoom to fit" and "Center here" follow.
pub fn show_add_menu_at(window: &Window, point: Point, at: Option<(f64, f64)>) {
    let Some(core) = core() else {
        return;
    };
    let config = core.host().config();
    let addable = crate::layout::view_edit::addable(core.host());
    let devices = crate::emulation::device_list();
    let items = Arc::new(if at.is_some() {
        view_menu::area_items(&config, &addable, &devices)
    } else {
        view_menu::add_items(&config, &addable, &devices)
    });
    let mut delegate = AddMenu::new(items.clone(), at);
    let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
        tracing::warn!("the add menu could not be created");
        return;
    };
    for (index, item) in items.iter().enumerate() {
        let Some(id) = id_of(ADD_FIRST_ID, index) else {
            break;
        };
        if item.action == Some(AddAction::ZoomToFit) {
            model.add_separator();
        }
        model.add_item(id, Some(&CefString::from(item.label.as_str())));
        model.set_enabled(id, i32::from(item.enabled()));
    }
    tracing::info!(
        x = point.x,
        y = point.y,
        rows = items.len(),
        "the add menu opens"
    );
    super::theme::paint_menu(&model);
    window.show_menu(Some(&mut model), Some(&point), MenuAnchorPosition::TOPLEFT);
}

wrap_button_delegate! {
    // The label strip of an empty cell (a hidden breakpoint, RB-080-41): a click opens the add
    // menu under it.
    struct HiddenLabel {
        height: i32,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            super::views::strip(0, self.height)
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, button: Option<&mut Button>) {
            let Some(button) = button else {
                return;
            };
            // `ui.viewContextMenu` off: the empty cell's label opens nothing; the toolbar's
            // "+" and the commands stay.
            if !core().is_some_and(|core| core.user_settings().ui.view_context_menu) {
                return;
            }
            let bounds = button.bounds_in_screen();
            let point = Point {
                x: bounds.x,
                y: bounds.y.saturating_add(bounds.height),
            };
            if let Some(window) = button.window() {
                show_add_menu_at(&window, point, None);
            }
        }
    }
}

/// The label strip of the empty cell of `breakpoint` (`hidden: <label>`), `height` DIP tall.
pub fn hidden_label(breakpoint: &str, height: u32) -> Option<LabelButton> {
    let text =
        core().map(|core| view_menu::hidden_cell_label(&core.host().config(), breakpoint))?;
    let mut delegate = HiddenLabel::new(i32::try_from(height).unwrap_or(i32::MAX));
    let button = label_button_create(Some(&mut delegate), Some(&CefString::from(text.as_str())))?;
    button.set_tooltip_text(Some(&CefString::from(
        rb_i18n::t(view_menu::KEY_ADD).as_str(),
    )));
    // For the right-click (`strips`).
    HIDDEN.with(|hidden| hidden.borrow_mut().push(button.clone()));
    Some(button)
}

/// Lets the strips of the empty cells go: the grid is built anew, its strips with it.
pub fn clear_hidden() {
    HIDDEN.with(|hidden| hidden.borrow_mut().clear());
}

/// Lets every label strip go (the main window was destroyed, RB-080-91): released while CEF
/// still runs, never by the thread's end after `CefShutdown`.
pub fn release() {
    LABELS.with(|labels| labels.borrow_mut().clear());
    HIDDEN.with(|hidden| hidden.borrow_mut().clear());
}
