//! The keyboard handlers of every browser: each key-down goes to the leader state machine
//! first (`OnPreKeyEvent`, RB-010-17); what rBrowser takes never reaches the page, and Chromium
//! drops the character events that follow it. Each handler names the focus its browser stands
//! for ([`KeySource`]): the shell, the canvas board (the shell's keys and its own `Ctrl+0`,
//! RB-080-94), a grid view, or DevTools (the dock and the DevTools window, RB-010-06), where
//! `shortcuts.disableInDevtools` lets every key through. A key no browser
//! handled - the focus on the window, a panel or an overlay without a browser - reaches the main
//! window's delegate, which hands it on as the shell's (`window.rs`). In a grid view Ctrl+0,
//! Ctrl+Plus and Ctrl+Minus the leader does not take zoom the page (RB-090-20,
//! `page_zoom.rs`); on the board Ctrl+0 stays the canvas's 100 %.

use std::os::raw::c_int;
use std::time::Instant;

use cef::*;
use rb_engine::ViewId;

use super::views::core;
use crate::control::KeyDown;
use crate::devtools::focus::KeySource;

/// The native event CEF hands `on_pre_key_event`: Windows' `MSG`.
#[cfg(windows)]
type OsEvent<'a> = Option<&'a mut sys::MSG>;
/// The native event CEF hands `on_pre_key_event`: an `NSEvent` pointer on macOS (RB-090-15).
#[cfg(target_os = "macos")]
type OsEvent<'a> = *mut u8;
/// The native event CEF hands `on_pre_key_event`: an X11 `XEvent` on Linux (RB-097-05).
#[cfg(target_os = "linux")]
type OsEvent<'a> = Option<&'a mut sys::XEvent>;

wrap_keyboard_handler! {
    // The keys of one view (`None`: the shell).
    pub struct ViewKeys {
        view: Option<ViewId>,
    }

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: OsEvent<'_>,
            _is_keyboard_shortcut: Option<&mut c_int>,
        ) -> c_int {
            let source = match self.view {
                None => KeySource::Shell,
                Some(_) => KeySource::View,
            };
            let taken = pre_key(source, event);
            // What the leader left of Chromium's zoom keys zooms the page (RB-090-20).
            match (taken, self.view, event) {
                (0, Some(view), Some(event)) => super::page_zoom::on_key(view, event),
                _ => taken,
            }
        }
    }
}

wrap_keyboard_handler! {
    // The keys of the canvas board (RB-080-94): the shell's, and `Ctrl+0` for 100 %.
    pub struct BoardKeys {}

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: OsEvent<'_>,
            _is_keyboard_shortcut: Option<&mut c_int>,
        ) -> c_int {
            pre_key(KeySource::Board, event)
        }
    }
}

wrap_keyboard_handler! {
    // The keys of DevTools' own browsers: the docked frontend and the DevTools window.
    pub struct DevtoolsKeys {}

    impl KeyboardHandler {
        fn on_pre_key_event(
            &self,
            _browser: Option<&mut Browser>,
            event: Option<&KeyEvent>,
            _os_event: OsEvent<'_>,
            _is_keyboard_shortcut: Option<&mut c_int>,
        ) -> c_int {
            pre_key(KeySource::Devtools, event)
        }
    }
}

/// Hands a key-down from `source` to the core; `1` when rBrowser takes it. Also the main
/// window's fallback for a focus outside every browser (`window.rs`, `on_key_event`).
pub(super) fn pre_key(source: KeySource, event: Option<&KeyEvent>) -> c_int {
    let (Some(event), Some(core)) = (event, core()) else {
        return 0;
    };
    let down = event.type_ == KeyEventType::RAWKEYDOWN || event.type_ == KeyEventType::KEYDOWN;
    // A page of the shell never zooms (RB-080-50): what the leader does not take of Chromium's
    // zoom keys ends here.
    let zoom = down
        && matches!(source, KeySource::Shell | KeySource::Board)
        && crate::shell::is_zoom_key(event.modifiers, event.windows_key_code);
    let Some(key) = KeyDown::from_event(
        down,
        event.modifiers,
        event.windows_key_code,
        event.character,
        event.unmodified_character,
    ) else {
        return c_int::from(zoom);
    };
    c_int::from(core.on_key_down(source.focus(), key, Instant::now()) || zoom)
}
