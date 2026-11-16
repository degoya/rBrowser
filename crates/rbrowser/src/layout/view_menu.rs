//! The two menus of RB-080-41, as rows without a window: the view menu of a label strip
//! ("Remove view", "Set as active", "Size...", "Pop out" / "Back into the grid", on the canvas
//! "Center on this view"; the engine rows beside them are [`crate::breakpoint_keys::engine_pick`])
//! and the add menu of a free area, an empty cell or the toolbar's "+" ("Add view" per hidden
//! breakpoint, "Add all", "New size...", on the free canvas "Zoom to fit" and "Center here").
//! The native menus of `cef_app/cell_labels.rs` and `cef_app/view_menus.rs` show them, in front
//! of the views in every mode: the canvas board and the toolbar ask for theirs
//! ([`MenuRequest`], RB-080-46). A row runs its command off
//! the calling thread ([`run_view_action`], [`run_add_action`]). `ui.viewContextMenu` off keeps
//! the native strips and the board without them; the "+" button and the commands stay. The view
//! menu and the strip's size list end in the switch of the footer under every view, set apart
//! after the engine submenu ([`menu_items`], [`size_list_items`], [`layout_rows`], RB-080-60).

use rb_config::{DevicePreset, ProjectConfig};
use rb_engine::{ViewHost as _, ViewId};
use serde_json::{Value, json};

use super::view_edit::{ADD, ADD_SIZE, ALL, REMOVE};
use crate::breakpoint_keys::sizes_public::{by_width, size_of};
use crate::control::Core;
use crate::host::Host;
use crate::page_window::ScreenRect;

/// The i18n keys of the rows.
pub const KEY_REMOVE: &str = "views.menu.remove";
pub const KEY_ACTIVATE: &str = "views.menu.activate";
pub const KEY_SIZE: &str = "views.menu.size";
pub const KEY_ENGINE: &str = "views.menu.engine";
/// The engine submenu of the right-click's view menu (RB-080-46).
pub const KEY_ENGINE_PICK: &str = "views.menu.engine_pick";
pub const KEY_POP_OUT: &str = "views.menu.pop_out";
pub const KEY_POP_IN: &str = "views.menu.pop_in";
pub const KEY_CENTER: &str = "views.menu.center";
/// The footer's check row (RB-080-60).
pub const KEY_FOOTER: &str = "views.menu.footer";
pub const KEY_ADD: &str = "views.menu.add";
pub const KEY_ADD_ALL: &str = "views.menu.add_all";
pub const KEY_NEW_SIZE: &str = "views.menu.new_size";
pub const KEY_ALL_OPEN: &str = "views.menu.all_open";
pub const KEY_ZOOM_TO_FIT: &str = "views.menu.zoom_to_fit";
pub const KEY_CENTER_HERE: &str = "views.menu.center_here";
/// The label strip of an empty cell; `{label}`.
pub const KEY_HIDDEN_CELL: &str = "views.hidden_cell";
/// A hidden breakpoint's row; `{label}`, `{width}`, `{height}`.
pub const KEY_ROW: &str = "sizes.pick.breakpoint";

/// Where a native menu opens: DIP from the top-left corner of the page that asked for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuAnchor {
    /// The canvas board.
    Board { x: i32, y: i32 },
    /// The shell view (the toolbar).
    Toolbar { x: i32, y: i32 },
    /// The status bar (RB-090-18).
    StatusBar { x: i32, y: i32 },
}

/// A native menu a page of the shell asked for (RB-080-46, `layout::view_menu_open`): the
/// window shows it in front of the views (`cef_app/view_menus.rs`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MenuRequest {
    /// The menu of `view`; with `canvas` its "Center on this view".
    View {
        view: ViewId,
        anchor: MenuAnchor,
        canvas: bool,
    },
    /// The add menu; `at` is the canvas point of a click on the free board, where a new view
    /// goes and "Center here" centres - with it the menu has the canvas's rows.
    Add {
        anchor: MenuAnchor,
        at: Option<(f64, f64)>,
    },
    /// The canvas zoom's presets above the status bar's level button (RB-090-18,
    /// [`super::zoom_menu`]); `level` is the canvas's, for the check.
    Zoom {
        anchor: MenuAnchor,
        level: Option<f64>,
    },
}

/// What a label strip of the grid is, for a right-click on it (RB-080-46).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strip {
    /// The strip of a view: its menu opens.
    View(ViewId),
    /// The strip of an empty cell (a hidden breakpoint): the add menu opens.
    Hidden,
}

/// The strip under the screen point `x`/`y` (DIP): the first of `strips` whose rectangle holds
/// it. CEF's label buttons report a press without its mouse button, so the window hears the
/// right button itself (`cef_app/strip_clicks.rs`) and asks here which strip it hit.
pub fn strip_at(x: i32, y: i32, strips: &[(Strip, ScreenRect)]) -> Option<Strip> {
    let holds = |rect: &ScreenRect| {
        rect.width > 0
            && rect.height > 0
            && (rect.x..rect.x.saturating_add(rect.width)).contains(&x)
            && (rect.y..rect.y.saturating_add(rect.height)).contains(&y)
    };
    strips
        .iter()
        .find(|(_, rect)| holds(rect))
        .map(|(strip, _)| *strip)
}

/// What a row of the view menu does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewAction {
    /// `view.remove`.
    Remove,
    /// The view becomes the active one.
    Activate,
    /// The size quick pick opens (the caller shows it).
    Size,
    /// `view.pop_out`.
    PopOut,
    /// `view.pop_in`.
    PopIn,
    /// Canvas: `layout.center { view }`.
    Center,
    /// `ui.view_footer.toggle`: the footer under every view (RB-080-60).
    Footer,
}

/// One row of the view menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewMenuItem {
    /// What it does.
    pub action: ViewAction,
    /// The text (already translated).
    pub label: String,
    /// A check row's mark; `None` for a plain row.
    pub checked: Option<bool>,
}

/// The rows of the view menu of `view`: on the canvas with "Center on this view".
pub fn view_items(host: &Host, view: ViewId, canvas: bool) -> Vec<ViewMenuItem> {
    let row = |action, key: &str| ViewMenuItem {
        action,
        label: rb_i18n::t(key),
        checked: None,
    };
    let mut items = vec![row(ViewAction::Remove, KEY_REMOVE)];
    if host.active_view() != Some(view) {
        items.push(row(ViewAction::Activate, KEY_ACTIVATE));
    }
    items.push(row(ViewAction::Size, KEY_SIZE));
    if host.is_detached(view) {
        items.push(row(ViewAction::PopIn, KEY_POP_IN));
    } else {
        items.push(row(ViewAction::PopOut, KEY_POP_OUT));
    }
    if canvas {
        items.push(row(ViewAction::Center, KEY_CENTER));
    }
    items
}

/// The rows of the view menu a right-click opens: [`view_items`], then "Footer under every view",
/// checked while `ui.viewFooter.visible` shows it (drawn last, set apart: [`layout_rows`]).
pub fn menu_items(core: &Core, view: ViewId, canvas: bool) -> Vec<ViewMenuItem> {
    let mut items = view_items(core.host(), view, canvas);
    if core.has_command(crate::view_footer::defs::TOGGLE) {
        items.push(ViewMenuItem {
            action: ViewAction::Footer,
            label: rb_i18n::t(KEY_FOOTER),
            checked: Some(core.user_settings().ui.view_footer.visible),
        });
    }
    items
}

/// The view rows of the size list a click on a strip opens (RB-080-41): [`menu_items`] without
/// "Size..." - the sizes are that menu; none with `ui.viewContextMenu` off.
pub fn size_list_items(core: &Core, view: ViewId) -> Vec<ViewMenuItem> {
    if !core.user_settings().ui.view_context_menu {
        return Vec::new();
    }
    menu_items(core, view, false)
        .into_iter()
        .filter(|item| item.action != ViewAction::Size)
        .collect()
}

/// Where a native menu draws a part of a view menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuRow {
    /// The row at this index of the items.
    Item(usize),
    /// The view's engine submenu.
    Engines,
    /// A separator.
    Separator,
}

/// The order a native menu draws `items` in: the view's own rows, its engine submenu when it
/// has one, then - after a separator - the rows for every view (the footer switch), so no
/// per-view entry stands under the switch for all views.
pub fn layout_rows(items: &[ViewMenuItem], engines: bool) -> Vec<MenuRow> {
    let (global, own): (Vec<_>, Vec<_>) = items
        .iter()
        .enumerate()
        .partition(|(_, item)| item.action == ViewAction::Footer);
    let mut rows: Vec<MenuRow> = own
        .into_iter()
        .map(|(index, _)| MenuRow::Item(index))
        .collect();
    if engines {
        rows.push(MenuRow::Engines);
    }
    if !global.is_empty() {
        rows.push(MenuRow::Separator);
        rows.extend(global.into_iter().map(|(index, _)| MenuRow::Item(index)));
    }
    rows
}

/// Runs `action` for `view` off the calling (UI) thread; `Size` is the caller's (the quick pick
/// hangs from its strip).
pub fn run_view_action(core: &Core, view: ViewId, action: ViewAction) {
    let (id, args) = match action {
        ViewAction::Remove => (REMOVE, json!({ "view": view })),
        ViewAction::Activate => {
            if let Err(error) = core.host().set_active_view(view) {
                tracing::warn!(%view, %error, "the view could not be activated");
            }
            return;
        }
        ViewAction::Size => return,
        ViewAction::PopOut => (crate::pop_out::POP_OUT, json!({ "view": view })),
        ViewAction::PopIn => (crate::pop_out::POP_IN, json!({ "view": view })),
        ViewAction::Center => (super::defs::CENTER, json!({ "view": view })),
        ViewAction::Footer => (crate::view_footer::defs::TOGGLE, Value::Null),
    };
    spawn(core, id, args);
}

/// What a row of the add menu does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddAction {
    /// `view.add { breakpoint }`.
    Add(String),
    /// `view.add { breakpoint: "all" }`.
    AddAll,
    /// `view.add_size` without a size: the picker.
    NewSize,
    /// Canvas: `layout.zoom_to_fit`.
    ZoomToFit,
    /// Canvas: `layout.center` on the clicked point.
    CenterHere,
}

/// One row of the add menu; a row without an action is the disabled "All breakpoints are open".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddMenuItem {
    /// What it does; `None` for the disabled note.
    pub action: Option<AddAction>,
    /// The text (already translated).
    pub label: String,
}

impl AddMenuItem {
    /// Whether the row can be chosen.
    pub fn enabled(&self) -> bool {
        self.action.is_some()
    }
}

/// The rows of the add menu for `config`: one per breakpoint of `addable` (the hidden ones
/// with a cell to open, [`super::view_edit::addable`]) by width (its label and size), "Add all"
/// when more than one, "New size..."; "All breakpoints are open" when none.
pub fn add_items(
    config: &ProjectConfig,
    addable: &[String],
    devices: &[DevicePreset],
) -> Vec<AddMenuItem> {
    let mut items: Vec<AddMenuItem> = by_width(&config.breakpoints, devices)
        .into_iter()
        .filter(|breakpoint| addable.contains(&breakpoint.id))
        .map(|breakpoint| {
            let (width, height) = size_of(breakpoint, devices);
            AddMenuItem {
                action: Some(AddAction::Add(breakpoint.id.clone())),
                label: rb_i18n::t_with(
                    KEY_ROW,
                    &[
                        (
                            "label",
                            breakpoint.label.as_deref().unwrap_or(&breakpoint.id),
                        ),
                        ("width", &width.to_string()),
                        ("height", &height.to_string()),
                    ],
                ),
            }
        })
        .collect();
    if items.is_empty() {
        items.push(AddMenuItem {
            action: None,
            label: rb_i18n::t(KEY_ALL_OPEN),
        });
    } else if items.len() > 1 {
        items.push(AddMenuItem {
            action: Some(AddAction::AddAll),
            label: rb_i18n::t(KEY_ADD_ALL),
        });
    }
    items.push(AddMenuItem {
        action: Some(AddAction::NewSize),
        label: rb_i18n::t(KEY_NEW_SIZE),
    });
    items
}

/// The rows of the add menu of the free canvas: [`add_items`], then "Zoom to fit" and "Center
/// here" (the caller draws a separator before the last two).
pub fn area_items(
    config: &ProjectConfig,
    addable: &[String],
    devices: &[DevicePreset],
) -> Vec<AddMenuItem> {
    let mut items = add_items(config, addable, devices);
    for (action, key) in [
        (AddAction::ZoomToFit, KEY_ZOOM_TO_FIT),
        (AddAction::CenterHere, KEY_CENTER_HERE),
    ] {
        items.push(AddMenuItem {
            action: Some(action),
            label: rb_i18n::t(key),
        });
    }
    items
}

/// Runs `action` off the calling (UI) thread; on the canvas `at` (canvas DIP) is where a new
/// view goes and what "Center here" centres on.
pub fn run_add_action(core: &Core, action: &AddAction, at: Option<(f64, f64)>) {
    let (id, mut args) = match action {
        AddAction::Add(breakpoint) => (ADD, json!({ "breakpoint": breakpoint })),
        AddAction::AddAll => (ADD, json!({ "breakpoint": ALL })),
        AddAction::NewSize => (ADD_SIZE, Value::Null),
        AddAction::ZoomToFit => ("layout.zoom_to_fit", Value::Null),
        AddAction::CenterHere => (super::defs::CENTER, json!({})),
    };
    // "Add all" places its views itself; the picker takes no point.
    let placed = matches!(action, AddAction::Add(_) | AddAction::CenterHere);
    if let (true, Some((x, y)), Some(object)) = (placed, at, args.as_object_mut()) {
        object.insert("x".to_owned(), json!(x));
        object.insert("y".to_owned(), json!(y));
    }
    spawn(core, id, args);
}

/// The text of the label strip of an empty cell: `hidden: <label>`.
pub fn hidden_cell_label(config: &ProjectConfig, breakpoint: &str) -> String {
    let label = config
        .breakpoints
        .iter()
        .find(|candidate| candidate.id == breakpoint)
        .and_then(|candidate| candidate.label.as_deref())
        .unwrap_or(breakpoint);
    rb_i18n::t_with(KEY_HIDDEN_CELL, &[("label", label)])
}

fn spawn(core: &Core, id: &str, args: Value) {
    let command = id.to_owned();
    core.run_command_async(id, args, move |result| {
        if let Err(error) = result {
            tracing::warn!(%command, %error, "a view menu command failed");
        }
    });
}

#[cfg(test)]
#[path = "view_menu_tests.rs"]
mod tests;
