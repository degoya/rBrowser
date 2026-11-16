//! The toolbar's pulldowns as native menus (RB-090-41): the theme, the environment, the layout
//! preset, the bookmarks and the overflow name their button's rectangle and their rows
//! (`nativeMenu`), and the core shows them as a native menu painted in rBrowser's theme
//! (`cef_app/native_menu.rs`), like the canvas zoom's presets (RB-090-18) and the view menus
//! (RB-080-46). A `<select>`'s list was Chromium's popup: it followed the theme only in part,
//! changed its colours under the mouse and could stand out of the window. The menu hangs below
//! the button towards the window's middle ([`placement`]); the menu runner keeps it on the
//! screen. Once it closed the shell hears the row chosen (`nativeMenuClosed`) and runs it
//! through the one command registry itself.
//!
//! The rows come from the shell's page and are checked here ([`NativeMenu::checked`]): the name
//! of a known pulldown ([`PULLDOWNS`], RB-090-45), at most [`MAX_ROWS`] rows, ids unique, texts
//! bounded. Nothing here runs a command.

use std::sync::Mutex;

use rb_commands::CommandError;
use rb_ipc::{CoreToShell, IpcErrorCode, MapRect, NativeMenuRow, ShellToCore};

use super::canvas_board::checked;
use crate::control::Core;
use crate::ipc::Failure;

/// The most rows of one menu (a project's bookmarks are the longest list).
pub const MAX_ROWS: usize = 200;
/// The longest text of a row, its id or its label, in characters.
pub const MAX_TEXT: usize = 256;
/// The longest name of a menu.
pub const MAX_MENU: usize = 64;
/// The pulldowns that may ask for a native menu: the toolbar's theme (RB-090-41), environment,
/// layout preset, bookmarks and overflow (RB-090-45). The shell names each instance
/// `<kind>-<n>` (`ui/src/toolbar/nativeMenu.ts`).
pub const PULLDOWNS: [&str; 5] = ["theme", "environment", "layout", "bookmarks", "overflow"];
/// The widest or tallest button, in DIP.
const MAX_EXTENT: u32 = 4096;

/// What a [`crate::host::ui::UiBackend::show_native_menu`] calls once the menu closed: the id
/// of the row chosen, `None` without a choice.
pub type NativeMenuDone = Box<dyn FnOnce(Option<String>) + Send>;

/// A menu the shell asked for, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeMenu {
    /// What the shell knows it by.
    pub menu: String,
    /// The button, in DIP from the shell view's top-left corner.
    pub anchor: MapRect,
    /// The rows.
    pub rows: Vec<NativeMenuRow>,
}

impl NativeMenu {
    /// The request's menu, or why it cannot be drawn.
    pub fn checked(
        menu: &str,
        anchor: MapRect,
        rows: &[NativeMenuRow],
    ) -> Result<Self, CommandError> {
        let invalid = |why: &str| Err(CommandError::InvalidArgs(format!("a native menu {why}")));
        let named = !menu.is_empty()
            && menu.len() <= MAX_MENU
            && menu
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte));
        if !named {
            return invalid("is named by 1 to 64 letters, digits and `-_.:`");
        }
        if !is_pulldown(menu) {
            return invalid(
                "belongs to a toolbar pulldown (theme, environment, layout, bookmarks, overflow)",
            );
        }
        checked(anchor.x, anchor.y)?;
        if anchor.width > MAX_EXTENT || anchor.height > MAX_EXTENT {
            return invalid("hangs from a button of at most 4096 DIP");
        }
        if rows.is_empty() || rows.len() > MAX_ROWS {
            return invalid("has 1 to 200 rows");
        }
        let mut ids: Vec<&str> = Vec::new();
        for row in rows {
            let (id, label) = match row {
                NativeMenuRow::Item { id, label, .. } => (Some(id.as_str()), label),
                NativeMenuRow::Heading { label } => (None, label),
                NativeMenuRow::Separator => continue,
            };
            if label.chars().count() > MAX_TEXT {
                return invalid("has labels of at most 256 characters");
            }
            if let Some(id) = id {
                if id.is_empty() || id.chars().count() > MAX_TEXT || ids.contains(&id) {
                    return invalid("has row ids of 1 to 256 characters, each once");
                }
                ids.push(id);
            }
        }
        if ids.is_empty() {
            return invalid("has a row to choose");
        }
        Ok(Self {
            menu: menu.to_owned(),
            anchor,
            rows: rows.to_vec(),
        })
    }
}

/// Whether `menu` names a pulldown of [`PULLDOWNS`]: its kind alone or `<kind>-<n>`.
pub fn is_pulldown(menu: &str) -> bool {
    let kind = match menu.rsplit_once('-') {
        Some((kind, n)) if !n.is_empty() && n.bytes().all(|byte| byte.is_ascii_digit()) => kind,
        Some(_) => return false,
        None => menu,
    };
    PULLDOWNS.contains(&kind)
}

/// The shell's request (`crate::features` through the board's and the view menus' chain);
/// `None` for every other message.
pub fn ipc(core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    let ShellToCore::NativeMenu { menu, anchor, rows } = message else {
        return None;
    };
    Some(
        open(core, menu, *anchor, rows)
            .map(|()| CoreToShell::Ack)
            .map_err(|error| {
                tracing::warn!(%error, "a native menu was not opened");
                (IpcErrorCode::BadRequest, error.to_string())
            }),
    )
}

fn open(
    core: &Core,
    menu: &str,
    anchor: MapRect,
    rows: &[NativeMenuRow],
) -> Result<(), CommandError> {
    let request = NativeMenu::checked(menu, anchor, rows)?;
    tracing::info!(
        menu,
        rows = rows.len(),
        "the toolbar asked for a native menu"
    );
    let me = core.arc();
    let name = request.menu.clone();
    core.ui().show_native_menu(
        request,
        Box::new(move |item| {
            if let Some(core) = me {
                core.send(&CoreToShell::NativeMenuClosed { menu: name, item });
            }
        }),
    );
    Ok(())
}

/// Which corner of the menu hangs at the point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
}

/// Where a menu hangs, in DIP from the shell view's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub corner: Corner,
}

/// Below the button: its left end while the button's middle is in the left half of a view
/// `view_width` wide, else its right end with the menu's top-right corner - the menu opens
/// towards the window's middle.
pub fn placement(anchor: MapRect, view_width: i32) -> Placement {
    let width = i32::try_from(anchor.width).unwrap_or(i32::MAX);
    let height = i32::try_from(anchor.height).unwrap_or(i32::MAX);
    let middle = anchor.x.saturating_add(width / 2);
    let y = anchor.y.saturating_add(height);
    if view_width > 0 && middle > view_width / 2 {
        Placement {
            x: anchor.x.saturating_add(width),
            y,
            corner: Corner::TopRight,
        }
    } else {
        Placement {
            x: anchor.x,
            y,
            corner: Corner::TopLeft,
        }
    }
}

/// `text` as a native menu shows it: `&` marks a mnemonic there, so a literal one is doubled,
/// and a tab or a line break (a tab starts the shortcut column) is a space.
pub fn menu_label(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .replace('&', "&&")
}

/// One row as the menu draws it; `index` is the row's place in the request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Item {
        index: usize,
        label: String,
        checked: Option<bool>,
        enabled: bool,
    },
    /// Drawn as a row that cannot be chosen.
    Heading {
        index: usize,
        label: String,
    },
    Separator,
}

/// The rows as the menu draws them.
pub fn entries(rows: &[NativeMenuRow]) -> Vec<Entry> {
    rows.iter()
        .enumerate()
        .map(|(index, row)| match row {
            NativeMenuRow::Item {
                label,
                checked,
                disabled,
                ..
            } => Entry::Item {
                index,
                label: menu_label(label),
                checked: *checked,
                enabled: !disabled,
            },
            NativeMenuRow::Heading { label } => Entry::Heading {
                index,
                label: menu_label(label),
            },
            NativeMenuRow::Separator => Entry::Separator,
        })
        .collect()
}

/// A menu that is open: the row chosen is kept ([`Self::choose`]) and told once, when the menu
/// closed ([`Self::close`]) - CEF runs the choice first and posts the close after it.
pub struct Pending {
    rows: Vec<NativeMenuRow>,
    chosen: Mutex<Option<String>>,
    done: Mutex<Option<NativeMenuDone>>,
}

impl Pending {
    pub fn new(rows: Vec<NativeMenuRow>, done: NativeMenuDone) -> Self {
        Self {
            rows,
            chosen: Mutex::new(None),
            done: Mutex::new(Some(done)),
        }
    }

    /// Row `index` was chosen; a heading, a disabled row or no row at all chooses nothing.
    pub fn choose(&self, index: usize) {
        if let Some(NativeMenuRow::Item {
            id,
            disabled: false,
            ..
        }) = self.rows.get(index)
        {
            *rb_util::lock(&self.chosen) = Some(id.clone());
        }
    }

    /// The menu closed: the row chosen, if any, is told - once.
    pub fn close(&self) {
        let done = rb_util::lock(&self.done).take();
        if let Some(done) = done {
            let item = rb_util::lock(&self.chosen).take();
            done(item);
        }
    }
}

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;
