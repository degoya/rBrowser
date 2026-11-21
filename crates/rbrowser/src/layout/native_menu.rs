//! The toolbar's pulldowns as native menus (RB-090-41): the theme, the environment, the layout
//! preset, the bookmarks, the overflow and the portal's button (RB-090-62) name their button's
//! rectangle and their rows (`nativeMenu`), and the core shows them as a native menu painted in
//! rBrowser's theme (`cef_app/native_menu.rs`), like the canvas zoom's presets (RB-090-18) and
//! the view menus (RB-080-46). A `<select>`'s list was Chromium's popup: it followed the theme
//! only in part, changed its colours under the mouse and could stand out of the window. The menu
//! hangs below the button towards the window's middle ([`placement`]); the menu runner keeps it
//! on the screen. Once it closed the shell hears the row chosen (`nativeMenuClosed`) and runs it
//! through the one command registry itself.
//!
//! The rows come from the shell's page and are checked here ([`NativeMenu::checked`]): the name
//! of a known pulldown ([`PULLDOWNS`], RB-090-45), at most [`MAX_ROWS`] rows per menu and
//! [`MAX_TOTAL`] in all, submenus at most [`MAX_DEPTH`] deep (RB-090-63), ids unique across the
//! whole tree, texts bounded, a radio row checked or not, a shortcut in rb-commands' text form.
//! Nothing here runs a command. How the rows are drawn - submenus, radio rows, the shortcut
//! beside a row - is [`super::native_menu_entries`].

use std::collections::HashSet;

use rb_commands::{Binding, CommandError};
use rb_ipc::{CoreToShell, IpcErrorCode, MapRect, NativeMenuRow, ShellToCore};

use super::canvas_board::checked;
pub use super::native_menu_entries::{Accelerator, Entry, Pending, entries, menu_label};
use crate::control::Core;
use crate::ipc::Failure;

/// The most rows of one menu or submenu (a project's bookmarks are the longest list).
pub const MAX_ROWS: usize = 200;
/// The most rows of a menu with all its submenus: the CEF command ids `27 100` to `27 899`
/// (`cef_app/native_menu.rs`), one per row.
pub const MAX_TOTAL: usize = 800;
/// The deepest a row may hang: the menu itself is level 1, `≡ › Tools › Network ›` level 3.
pub const MAX_DEPTH: usize = 4;
/// The longest shortcut text beside a row, in characters.
pub const MAX_ACCEL: usize = 64;
/// The longest text of a row, its id or its label, in characters.
pub const MAX_TEXT: usize = 256;
/// The longest name of a menu.
pub const MAX_MENU: usize = 64;
/// The pulldowns that may ask for a native menu: the toolbar's theme (RB-090-41), environment,
/// layout preset, bookmarks and overflow (RB-090-45), the portal's button (RB-090-62), the
/// always-visible main menu `≡` and the split buttons DevTools, Screenshot, Video and Checks
/// (RB-090-63), and the toolbar's "+" with the devices (RB-094-20). The shell names each
/// instance `<kind>-<n>` (`ui/src/toolbar/nativeMenu.ts`).
pub const PULLDOWNS: [&str; 12] = [
    "add",
    "theme",
    "environment",
    "layout",
    "bookmarks",
    "overflow",
    "portal",
    "menu",
    "devtools",
    "capture",
    "video",
    "checks",
];
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
                "belongs to a toolbar pulldown (theme, environment, layout, bookmarks, overflow, \
                 portal, menu, devtools, capture, video, checks)",
            );
        }
        checked(anchor.x, anchor.y)?;
        if anchor.width > MAX_EXTENT || anchor.height > MAX_EXTENT {
            return invalid("hangs from a button of at most 4096 DIP");
        }
        let mut tree = Tree::default();
        if let Err(why) = tree.walk(rows, 1) {
            return invalid(why);
        }
        if tree.ids.is_empty() {
            return invalid("has a row to choose");
        }
        Ok(Self {
            menu: menu.to_owned(),
            anchor,
            rows: rows.to_vec(),
        })
    }
}

/// The rows of a menu as [`NativeMenu::checked`] walks them: the ids seen, the rows counted.
#[derive(Default)]
struct Tree<'a> {
    ids: HashSet<&'a str>,
    total: usize,
}

impl<'a> Tree<'a> {
    /// Checks `rows`, a menu at `depth` (the menu itself is 1), and the submenus in it.
    fn walk(&mut self, rows: &'a [NativeMenuRow], depth: usize) -> Result<(), &'static str> {
        if depth > MAX_DEPTH {
            return Err("nests submenus at most 4 deep");
        }
        if rows.is_empty() || rows.len() > MAX_ROWS {
            return Err("has 1 to 200 rows in each menu");
        }
        for row in rows {
            self.total += 1;
            if self.total > MAX_TOTAL {
                return Err("has at most 800 rows in all");
            }
            match row {
                NativeMenuRow::Item {
                    id,
                    label,
                    checked,
                    radio,
                    accel,
                    ..
                } => {
                    bounded(label)?;
                    if id.is_empty()
                        || id.chars().count() > MAX_TEXT
                        || !self.ids.insert(id.as_str())
                    {
                        return Err("has row ids of 1 to 256 characters, each once");
                    }
                    if *radio && checked.is_none() {
                        return Err("has radio rows that say whether they are chosen");
                    }
                    if let Some(accel) = accel
                        && (accel.chars().count() > MAX_ACCEL || accel.parse::<Binding>().is_err())
                    {
                        return Err("has shortcuts in the text form of the command registry");
                    }
                }
                NativeMenuRow::Heading { label } => bounded(label)?,
                NativeMenuRow::Separator => {}
                NativeMenuRow::Submenu { label, rows } => {
                    bounded(label)?;
                    self.walk(rows, depth + 1)?;
                }
            }
        }
        Ok(())
    }
}

fn bounded(label: &str) -> Result<(), &'static str> {
    if label.chars().count() > MAX_TEXT {
        return Err("has labels of at most 256 characters");
    }
    Ok(())
}

/// Whether `menu` names a pulldown of [`PULLDOWNS`]: its kind alone or `<kind>-<n>`.
pub fn is_pulldown(menu: &str) -> bool {
    kind_of(menu).is_some_and(|kind| PULLDOWNS.contains(&kind))
}

/// The kind of `<kind>-<n>` or of a bare kind; `None` for a malformed name.
fn kind_of(menu: &str) -> Option<&str> {
    match menu.rsplit_once('-') {
        Some((kind, n)) if !n.is_empty() && n.bytes().all(|byte| byte.is_ascii_digit()) => {
            Some(kind)
        }
        Some(_) => None,
        None => Some(menu),
    }
}

/// The menus that list the devices (RB-094-20): the main menu and the toolbar's "+". Opening
/// one takes the device list again in the background, for the next opening.
pub fn lists_devices(menu: &str) -> bool {
    matches!(kind_of(menu), Some("menu" | "add"))
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
    if lists_devices(menu) {
        crate::engines::android::menu::refresh(core);
    }
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

#[cfg(test)]
#[path = "native_menu_tests.rs"]
mod tests;
