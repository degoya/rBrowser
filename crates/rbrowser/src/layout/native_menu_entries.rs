//! How the rows of a toolbar pulldown's native menu are drawn ([`super::native_menu`]): a row
//! with its check or radio mark (RB-090-41, RB-090-63), a heading as a row that cannot be
//! chosen, a line, a submenu with its own rows. A command's shortcut stands beside its row: a
//! direct combination (`Ctrl+Shift+1`) as the menu's own accelerator, right-aligned and muted; a
//! leader chord (`Ctrl+Shift+Space L G`), which no accelerator can show, after the label.
//!
//! Every row has an index, its place in the tree read depth first - a submenu before its rows -
//! and the menu runner gives row `index` the command id `FIRST_ID + index`
//! (`cef_app/native_menu.rs`). [`Pending`] keeps the row chosen and tells it once the menu
//! closed.

use std::sync::Mutex;

use rb_commands::{Binding, keymap::windows_key_code};
use rb_ipc::NativeMenuRow;

use super::native_menu::NativeMenuDone;

/// The gap between a label and a leader chord after it: two em spaces.
const CHORD_GAP: &str = "\u{2003}\u{2003}";

/// A direct shortcut as the menu's accelerator: a Windows virtual-key code and the modifiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accelerator {
    pub key_code: i32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

/// One row as the menu draws it; `index` is the row's place in the tree, read depth first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    Item {
        index: usize,
        label: String,
        checked: Option<bool>,
        /// One of a set of choices: drawn as a radio row.
        radio: bool,
        /// A direct shortcut beside the row; a leader chord is in `label` instead.
        accelerator: Option<Accelerator>,
        enabled: bool,
    },
    /// Drawn as a row that cannot be chosen.
    Heading {
        index: usize,
        label: String,
    },
    Separator,
    /// A row that opens `entries`.
    Submenu {
        index: usize,
        label: String,
        entries: Vec<Entry>,
    },
}

/// `text` as a native menu shows it: `&` marks a mnemonic there, so a literal one is doubled,
/// and a tab or a line break (a tab starts the shortcut column) is a space.
pub fn menu_label(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .replace('&', "&&")
}

/// The accelerator of a direct shortcut; `None` for a leader chord, a shortcut with the Windows
/// key and text that is no shortcut.
pub fn accelerator(text: &str) -> Option<Accelerator> {
    let Ok(Binding::Direct(shortcut)) = text.parse::<Binding>() else {
        return None;
    };
    let modifiers = shortcut.modifiers;
    if modifiers.meta {
        return None;
    }
    Some(Accelerator {
        key_code: i32::try_from(windows_key_code(shortcut.key)?).ok()?,
        shift: modifiers.shift,
        ctrl: modifiers.ctrl,
        alt: modifiers.alt,
    })
}

/// A row's label and accelerator: a shortcut the menu cannot show as its accelerator (a leader
/// chord) after the label.
fn labelled(label: &str, accel: Option<&str>) -> (String, Option<Accelerator>) {
    let shown = menu_label(label);
    match accel {
        None => (shown, None),
        Some(text) => match accelerator(text) {
            Some(native) => (shown, Some(native)),
            None => (format!("{shown}{CHORD_GAP}{}", menu_label(text)), None),
        },
    }
}

/// The rows as the menu draws them.
pub fn entries(rows: &[NativeMenuRow]) -> Vec<Entry> {
    let mut next = 0;
    build(rows, &mut next)
}

fn build(rows: &[NativeMenuRow], next: &mut usize) -> Vec<Entry> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let index = *next;
        *next += 1;
        out.push(match row {
            NativeMenuRow::Item {
                label,
                checked,
                radio,
                accel,
                disabled,
                ..
            } => {
                let (label, accelerator) = labelled(label, accel.as_deref());
                Entry::Item {
                    index,
                    label,
                    checked: *checked,
                    radio: *radio,
                    accelerator,
                    enabled: !disabled,
                }
            }
            NativeMenuRow::Heading { label } => Entry::Heading {
                index,
                label: menu_label(label),
            },
            NativeMenuRow::Separator => Entry::Separator,
            NativeMenuRow::Submenu { label, rows } => Entry::Submenu {
                index,
                label: menu_label(label),
                entries: build(rows, next),
            },
        });
    }
    out
}

/// The id of every row that can be chosen, by index; `None` for the others.
fn choices(rows: &[NativeMenuRow], out: &mut Vec<Option<String>>) {
    for row in rows {
        match row {
            NativeMenuRow::Item {
                id,
                disabled: false,
                ..
            } => out.push(Some(id.clone())),
            NativeMenuRow::Submenu { rows, .. } => {
                out.push(None);
                choices(rows, out);
            }
            _ => out.push(None),
        }
    }
}

/// A menu that is open: the row chosen is kept ([`Self::choose`]) and told once, when the menu
/// closed ([`Self::close`]) - CEF runs the choice first and posts the close after it.
pub struct Pending {
    choices: Vec<Option<String>>,
    chosen: Mutex<Option<String>>,
    done: Mutex<Option<NativeMenuDone>>,
}

impl Pending {
    pub fn new(rows: &[NativeMenuRow], done: NativeMenuDone) -> Self {
        let mut found = Vec::new();
        choices(rows, &mut found);
        Self {
            choices: found,
            chosen: Mutex::new(None),
            done: Mutex::new(Some(done)),
        }
    }

    /// Row `index` was chosen; a heading, a submenu, a disabled row or no row at all chooses
    /// nothing.
    pub fn choose(&self, index: usize) {
        if let Some(Some(id)) = self.choices.get(index) {
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
#[path = "native_menu_entries_tests.rs"]
mod tests;
