//! The macOS application object and its menu bar (RB-090-15).
//!
//! Chromium's message pump asks the shared `NSApplication` whether it is inside `sendEvent:`
//! (`CrAppProtocol`, `CrAppControlProtocol`; CEF names the set `CefAppProtocol`), so the shared
//! application must be of a class that answers: [`RbApplication`], made the shared one before
//! CEF starts ([`install`]). Its `terminate:` (the Quit item, Cmd+Q, the Dock's Quit, a log-out)
//! closes the main window as its close button does, so the app always ends through CEF's own
//! shutdown (`window.rs`, `on_window_destroyed`), never through `exit`.
//!
//! The menu bar has the standard items: the app menu (About, Settings, Hide, Quit; About and
//! Settings run the app's commands `app.about` and `settings.open`) and Edit (undo, redo, cut,
//! copy, paste, select all, sent down the responder chain to the focused page or field) and
//! Window (Minimize, Close Window: the key window's own `performMiniaturize:` and
//! `performClose:`). The titles come from the catalogue. Main thread only.

#![allow(
    unsafe_code,
    reason = "an Objective-C subclass of NSApplication: the class definition, a super call and the selectors of the menu items"
)]

use std::cell::Cell;

use cef::application_mac::{CefAppProtocol, CrAppControlProtocol, CrAppProtocol};
use cef::{ImplWindow, quit_message_loop};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool, NSObject, NSObjectProtocol, Sel};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEvent, NSMenu, NSMenuItem, NSResponder};
use objc2_foundation::NSString;
use rb_i18n::t;

use super::views::{UI, core};

thread_local! {
    /// Whether the application is inside `sendEvent:` (the main thread's alone).
    static HANDLING_SEND_EVENT: Cell<bool> = const { Cell::new(false) };
}

define_class!(
    // SAFETY: NSApplication may be subclassed; the class adds no instance variables and does
    // not implement Drop; `sendEvent:` calls the superclass's, `terminate:` ends the app by the
    // main window's close, which ends the message loop.
    #[unsafe(super(NSApplication, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "RbApplication"]
    pub struct RbApplication;

    impl RbApplication {
        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            let outer = HANDLING_SEND_EVENT.with(|handling| handling.replace(true));
            // SAFETY: NSApplication's own `sendEvent:` with the event this one was given.
            let _: () = unsafe { msg_send![super(self), sendEvent: event] };
            HANDLING_SEND_EVENT.with(|handling| handling.set(outer));
        }

        #[unsafe(method(terminate:))]
        fn terminate(&self, _sender: Option<&AnyObject>) {
            quit();
        }

        #[unsafe(method(rbAbout:))]
        fn about(&self, _sender: Option<&AnyObject>) {
            run_command(crate::about::COMMAND);
        }

        #[unsafe(method(rbOpenSettings:))]
        fn open_settings(&self, _sender: Option<&AnyObject>) {
            run_command(crate::settings_window::OPEN_COMMAND);
        }
    }

    unsafe impl NSObjectProtocol for RbApplication {}

    unsafe impl CrAppProtocol for RbApplication {
        #[unsafe(method(isHandlingSendEvent))]
        fn is_handling_send_event(&self) -> Bool {
            Bool::new(HANDLING_SEND_EVENT.with(Cell::get))
        }
    }

    unsafe impl CrAppControlProtocol for RbApplication {
        #[unsafe(method(setHandlingSendEvent:))]
        fn set_handling_send_event(&self, handling: Bool) {
            HANDLING_SEND_EVENT.with(|cell| cell.set(handling.as_bool()));
        }
    }

    unsafe impl CefAppProtocol for RbApplication {}
);

/// Makes [`RbApplication`] the shared application and gives it the menu bar. Before CEF starts
/// (`run_plain`), on the main thread.
pub fn install(main_thread: MainThreadMarker) {
    // SAFETY: `sharedApplication` sent to the subclass creates the process's shared application
    // as an instance of it (the first call in the process: nothing has asked for NSApp yet).
    let app: Retained<NSApplication> =
        unsafe { msg_send![RbApplication::class(), sharedApplication] };
    if !app.isKindOfClass(RbApplication::class()) {
        tracing::error!(
            "the shared application is not rBrowser's; Chromium cannot pump its events"
        );
    }
    let bar = menu_bar(main_thread);
    app.setMainMenu(Some(&*bar));
}

/// The menu bar: the app menu, Edit, then Window.
fn menu_bar(main_thread: MainThreadMarker) -> Retained<NSMenu> {
    let bar = NSMenu::new(main_thread);
    let app_menu = submenu(
        main_thread,
        "rBrowser",
        &[
            Some((t("commands.app.about"), sel!(rbAbout:), "")),
            None,
            Some((t("macos.menu.settings"), sel!(rbOpenSettings:), ",")),
            None,
            Some((t("macos.menu.hide"), sel!(hide:), "h")),
            None,
            Some((t("macos.menu.quit"), sel!(terminate:), "q")),
        ],
    );
    let edit_menu = submenu(
        main_thread,
        &t("macos.menu.edit"),
        &[
            Some((t("macos.menu.undo"), sel!(undo:), "z")),
            // An upper-case key equivalent adds Shift: Shift+Cmd+Z.
            Some((t("macos.menu.redo"), sel!(redo:), "Z")),
            None,
            Some((t("macos.menu.cut"), sel!(cut:), "x")),
            Some((t("macos.menu.copy"), sel!(copy:), "c")),
            Some((t("macos.menu.paste"), sel!(paste:), "v")),
            Some((t("macos.menu.select_all"), sel!(selectAll:), "a")),
        ],
    );
    // Close (Cmd+W) and Minimize (Cmd+M) go to the key window (RB-090-98).
    let window_menu = submenu(
        main_thread,
        &t("macos.menu.window"),
        &[
            Some((t("macos.menu.minimize"), sel!(performMiniaturize:), "m")),
            None,
            Some((t("macos.menu.close"), sel!(performClose:), "w")),
        ],
    );
    for menu in [app_menu, edit_menu, window_menu] {
        let holder = item(main_thread, "", None, "");
        holder.setSubmenu(Some(&*menu));
        bar.addItem(&holder);
    }
    bar
}

/// A menu titled `title` with `items` (`None`: a separator).
fn submenu(
    main_thread: MainThreadMarker,
    title: &str,
    items: &[Option<(String, Sel, &str)>],
) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(main_thread), &NSString::from_str(title));
    for entry in items {
        match entry {
            Some((title, action, key)) => {
                menu.addItem(&item(main_thread, title, Some(*action), key))
            }
            None => menu.addItem(&NSMenuItem::separatorItem(main_thread)),
        }
    }
    menu
}

/// A menu item with Cmd and `key` as its key equivalent (`""`: none).
fn item(
    main_thread: MainThreadMarker,
    title: &str,
    action: Option<Sel>,
    key: &str,
) -> Retained<NSMenuItem> {
    // SAFETY: each action is a selector that NSApplication, RbApplication or the responder
    // chain answers (`terminate:`, `hide:`, `rbAbout:`, `undo:`, `copy:`, NSWindow's
    // `performClose:` and `performMiniaturize:` and the like).
    unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(main_thread),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    }
}

/// Quit: the main window closes as by its close button (the browsers close, the message loop
/// ends after); without one the message loop ends at once.
fn quit() {
    let window = UI.with(|ui| ui.try_borrow().ok().and_then(|ui| ui.window.clone()));
    match window {
        Some(window) => window.close(),
        None => quit_message_loop(),
    }
}

/// Runs the app's command `id` from a menu item, off the main thread.
fn run_command(id: &'static str) {
    let Some(core) = core() else {
        return;
    };
    core.run_command_async(id, serde_json::json!({}), move |result| {
        if let Err(error) = result {
            tracing::warn!(command = id, %error, "the menu's command failed");
        }
    });
}
