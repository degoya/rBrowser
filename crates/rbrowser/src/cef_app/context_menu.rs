//! The context menu of a grid view: Chromium's own menu, with rBrowser's block below it
//! (RB-020-05, RB-080-14, RB-080-43) - the entries, their state and their commands are the
//! portable [`crate::context_menu`] model, built from the core's state when the menu opens;
//! this side only renders the items into CEF's `MenuModel` and maps a chosen command id back.
//! UI thread; every entry runs on the core's runtime ([`crate::context_menu::run`]), never here.
//!
//! The handler owns nothing (RB-080-45): CEF asks the client for it once per callback, so the
//! handler that gets the command is not the one that built the menu. The block a view showed
//! is the client's ([`crate::context_menu::ViewMenu`], a field of `ViewClient`), shared with
//! every handler it makes; it goes with the view.
//!
//! Command ids: the block takes `context_menu::model::FIRST_ID` (28 010) and up in CEF's user
//! range (`MENU_ID_USER_FIRST` 26500 to `MENU_ID_USER_LAST` 28500), numbered in menu order at
//! build time; its submenus take ids of their own below it (`FIRST_MENU_ID`, 27 900, a hundred
//! of them); the size quick pick's rows start at 26 500.

use std::os::raw::c_int;

use cef::*;

use super::views::core;
pub use crate::context_menu::ViewMenu;
use crate::context_menu::model::{Mark, plain_label};
use crate::context_menu::{Item, Label};

wrap_context_menu_handler! {
    // The context menu of one grid view; `menu` is the client's, shared (the module's note).
    pub struct ViewContextMenu {
        menu: ViewMenu,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            let (Some(model), Some(core)) = (model, core()) else {
                return;
            };
            // Without parameters there is no point: the point-based entries are disabled.
            let point = params.map(|params| (params.xcoord(), params.ycoord()));
            let items = self.menu.opened(core, point);
            // Chromium's Print has no use in a breakpoint view (owner, 2026-11-17).
            model.remove(MenuId::PRINT.get_raw());
            if model.count() > 0 && !items.is_empty() {
                model.add_separator();
            }
            render(model, &items);
            // rBrowser's theme, not the page's (RB-080-95).
            super::theme::paint_menu(model);
        }

        fn on_context_menu_command(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            command_id: c_int,
            _event_flags: EventFlags,
        ) -> c_int {
            let Some(core) = core() else {
                return 0;
            };
            c_int::from(self.menu.chosen(core, command_id))
        }
    }
}

impl ViewContextMenu {
    /// A handler of `menu`, the menu of a view's client.
    pub fn of(menu: &ViewMenu) -> ContextMenuHandler {
        Self::new(menu.clone())
    }
}

/// The radio group of a menu's choices. One id serves: each submenu is a model of its own,
/// and no menu of the block has two groups of choices.
const RADIO_GROUP: c_int = 1;

/// Adds `items` to `model`: entries with their enabled state and mark, separators, submenus.
fn render(model: &mut MenuModel, items: &[Item]) {
    for item in items {
        match item {
            Item::Entry(entry) => {
                let label = CefString::from(text(&entry.label).as_str());
                match entry.mark {
                    Mark::None => {
                        model.add_item(entry.id, Some(&label));
                    }
                    Mark::Check(checked) => {
                        model.add_check_item(entry.id, Some(&label));
                        model.set_checked(entry.id, c_int::from(checked));
                    }
                    Mark::Radio(chosen) => {
                        model.add_radio_item(entry.id, Some(&label), RADIO_GROUP);
                        model.set_checked(entry.id, c_int::from(chosen));
                    }
                }
                if !entry.enabled {
                    model.set_enabled(entry.id, 0);
                }
            }
            Item::Separator => {
                model.add_separator();
            }
            Item::Menu { id, label, items } => {
                let label = CefString::from(text(label).as_str());
                if let Some(mut submenu) = model.add_sub_menu(*id, Some(&label)) {
                    render(&mut submenu, items);
                }
            }
        }
    }
}

/// A label's text: the catalogue's as it is; anything from outside it (a plugin's title, a
/// preset's name, a time zone) through [`plain_label`], so an `&` in it is no mnemonic.
fn text(label: &Label) -> String {
    match label {
        Label::Key(key) => rb_i18n::t(key),
        Label::Text(text) => plain_label(text),
        Label::KeyOr { key, text } => {
            let found = rb_i18n::t(key);
            if found == *key {
                plain_label(text)
            } else {
                found
            }
        }
    }
}
