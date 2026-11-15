//! rBrowser's own theme in the native parts (RB-080-95, `crate::app_theme`): every top-level
//! window - main, pages, page windows, popped-out views - takes the theme's colours
//! (`cef_window_t::set_theme_color`, then `theme_changed`, which repaints its views: the grid's
//! ground, the label strips and their text; the footers and the leader's mark paint themselves
//! again from their `on_theme_changed`), its native menus their colours
//! (`cef_menu_model_t::set_color_at`, [`paint_menu`]) and its title bar dark mode
//! (`theme_dwm.rs`). `system` is resolved by Windows' app mode, read again whenever Windows
//! switches it (`on_theme_colors_changed`). The web pages of the shell follow `ui.theme` on their
//! own (`ui/src/theme/theme.ts`); the pages in the views never follow it. UI thread.

use std::cell::{Cell, RefCell};

use cef::sys::cef_color_id_t;
use cef::*;
use rb_config::Theme;

use crate::app_theme::{NativePalette, Resolved, opaque, palette, resolve};

thread_local! {
    /// `ui.theme`, as the host said last.
    static SETTING: Cell<Theme> = const { Cell::new(Theme::System) };
    /// Whether Windows uses the dark mode for apps, as read last.
    static SYSTEM_DARK: Cell<Option<bool>> = const { Cell::new(None) };
    /// Every top-level window created so far; a closed one is dropped at the next change.
    static WINDOWS: RefCell<Vec<Window>> = const { RefCell::new(Vec::new()) };
}

/// The window colours that carry the ground.
const GROUNDS: [cef_color_id_t; 3] = [
    cef_color_id_t::CEF_ColorPrimaryBackground,
    cef_color_id_t::CEF_ColorWindowBackground,
    cef_color_id_t::CEF_ColorDialogBackground,
];
/// The window colours that carry the text of labels and buttons.
const TEXTS: [cef_color_id_t; 3] = [
    cef_color_id_t::CEF_ColorLabelForeground,
    cef_color_id_t::CEF_ColorPrimaryForeground,
    cef_color_id_t::CEF_ColorButtonForeground,
];

/// `ui.theme` changed (or the core started): every window takes the theme at once.
pub fn set(theme: Theme) {
    SETTING.with(|setting| setting.set(theme));
    SYSTEM_DARK.with(|dark| dark.set(Some(super::theme_dwm::system_dark())));
    let theme = resolved();
    let colors = palette(theme);
    for window in windows() {
        color(&window, &colors);
        window.theme_changed();
        super::theme_dwm::set_dark_title_bar(&window, theme.is_dark());
    }
    super::frame::theme_changed(opaque(colors.ground));
    super::engine_badge::theme_changed();
}

/// A top-level window was created: it takes the theme before it is shown and follows every
/// change after. From each window's `on_window_created`.
pub fn adopt(window: &Window) {
    // The setting in effect, should the window come before the host's first word.
    if let Some(core) = super::views::core() {
        SETTING.with(|setting| setting.set(core.user_settings().ui.theme));
    }
    let theme = resolved();
    color(window, &palette(theme));
    window.theme_changed();
    super::theme_dwm::set_dark_title_bar(window, theme.is_dark());
    WINDOWS.with(|windows| windows.borrow_mut().push(window.clone()));
}

/// Windows (or CEF) changed the theme of `window` and reset its colours to the standard ones:
/// ours again - `system` is read anew. From each window's `on_theme_colors_changed`; the views'
/// `on_theme_changed` follow by themselves (CEF's documented order, no `theme_changed` here).
pub fn colors_changed(window: &Window) {
    SYSTEM_DARK.with(|dark| dark.set(Some(super::theme_dwm::system_dark())));
    let theme = resolved();
    color(window, &palette(theme));
    super::theme_dwm::set_dark_title_bar(window, theme.is_dark());
}

/// Gives a native menu the theme's colours, every row (`index -1`). Before each `show_menu`
/// and in the pages' context menus.
pub fn paint_menu(model: &MenuModel) {
    let colors = palette(resolved());
    let rows = [
        (MenuColorType::TEXT, colors.menu_text),
        (MenuColorType::TEXT_HOVERED, colors.menu_hover_text),
        (MenuColorType::TEXT_ACCELERATOR, colors.muted),
        (
            MenuColorType::TEXT_ACCELERATOR_HOVERED,
            colors.menu_hover_text,
        ),
        (MenuColorType::BACKGROUND, colors.menu_ground),
        (MenuColorType::BACKGROUND_HOVERED, colors.menu_hover_ground),
    ];
    for (kind, rgb) in rows {
        model.set_color_at(-1, kind, opaque(rgb));
    }
}

/// The text colour (ARGB) of dimmed text that must still be read (an engine badge whose
/// engine cannot run).
pub fn muted_text() -> u32 {
    opaque(palette(resolved()).muted)
}

/// The native colours of the theme drawn now (the leader's label strip, RB-090-22).
pub fn colors() -> NativePalette {
    palette(resolved())
}

/// Lets every window go: the main window was destroyed and CEF shuts down.
pub fn clear() {
    WINDOWS.with(|windows| windows.borrow_mut().clear());
}

/// The theme drawn now.
fn resolved() -> Resolved {
    let dark = SYSTEM_DARK.with(|dark| match dark.get() {
        Some(dark) => dark,
        None => {
            let read = super::theme_dwm::system_dark();
            dark.set(Some(read));
            read
        }
    });
    resolve(SETTING.with(Cell::get), dark)
}

/// The windows still open; the closed ones are dropped.
fn windows() -> Vec<Window> {
    WINDOWS.with(|windows| {
        let mut windows = windows.borrow_mut();
        windows.retain(|window| window.is_valid() != 0);
        windows.clone()
    })
}

/// Sets the theme colours of `window`: grounds, texts and the menus'.
fn color(window: &Window, colors: &NativePalette) {
    let set = |id: cef_color_id_t, rgb: u32| window.set_theme_color(id as i32, opaque(rgb));
    for id in GROUNDS {
        set(id, colors.ground);
    }
    for id in TEXTS {
        set(id, colors.text);
    }
    set(cef_color_id_t::CEF_ColorMenuBackground, colors.menu_ground);
    set(
        cef_color_id_t::CEF_ColorMenuItemForeground,
        colors.menu_text,
    );
    set(
        cef_color_id_t::CEF_ColorMenuItemForegroundDisabled,
        colors.muted,
    );
    set(
        cef_color_id_t::CEF_ColorMenuItemBackgroundSelected,
        colors.menu_hover_ground,
    );
    set(
        cef_color_id_t::CEF_ColorMenuItemForegroundSelected,
        colors.menu_hover_text,
    );
    set(cef_color_id_t::CEF_ColorMenuSeparator, colors.menu_line);
    set(cef_color_id_t::CEF_ColorMenuBorder, colors.menu_line);
}
