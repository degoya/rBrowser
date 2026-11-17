//! rBrowser's own theme, light or dark, everywhere (RB-080-95) - the shell, the panels, the
//! settings and page windows, and the native parts: the window ground, the label strips, the
//! native menus and the Windows title bar. Never the pages in the views: their colour scheme is
//! the emulation's (`emulation.toggle_color_scheme`, the toolbar's page sun and moon).
//!
//! | Part | Where |
//! | --- | --- |
//! | The setting `ui.theme` (system, light, dark) | `rb_config::Theme` |
//! | The commands `ui.theme.cycle` and `ui.theme.set`, the status bar's notice | [`defs`] |
//! | The native colours of each theme and their contrast | [`palette`] |
//! | The web pages of the shell (`dark` class, Nuxt UI) | `ui/src/theme/theme.ts` |
//! | The main menu's App > Theme (radio, RB-090-63) | `ui/src/menu/mainMenu.ts` |
//! | The windows: theme colours, menus, the title bar (`DwmSetWindowAttribute`) | `cef_app/theme.rs`, `cef_app/theme_dwm.rs` |
//!
//! The window follows the setting through [`crate::host::ui::UiBackend::set_app_theme`] at the
//! start and after every change of `ui.theme` (`Core::replace_user_settings`, RB-090-36),
//! without a restart.

pub mod defs;
pub mod palette;

pub use defs::{CYCLE, SET, register};
pub use palette::{
    ACTIVE_SIGN, DARK, LIGHT, NativePalette, Resolved, composite, marked, opaque, palette, resolve,
};

use rb_config::Theme;

use crate::control::Core;

/// The setting the commands write and the window follows.
pub const THEME_KEY: &str = "ui.theme";

/// The theme after `theme` in the toolbar's and the command's turn: system, light, dark.
#[must_use]
pub fn next(theme: Theme) -> Theme {
    match theme {
        Theme::System => Theme::Light,
        Theme::Light => Theme::Dark,
        Theme::Dark => Theme::System,
    }
}

/// The setting's word for `theme` (`system`, `light`, `dark`).
#[must_use]
pub fn word(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "system",
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}

/// `word` as a theme; `None` for anything else.
#[must_use]
pub fn parse(word: &str) -> Option<Theme> {
    match word {
        "system" => Some(Theme::System),
        "light" => Some(Theme::Light),
        "dark" => Some(Theme::Dark),
        _ => None,
    }
}

/// The core is up: the window takes the theme before anyone sees it.
pub fn on_start(core: &Core) {
    on_user_settings_changed(core);
}

/// `ui.theme` changed: every window follows it at once.
pub fn on_user_settings_changed(core: &Core) {
    core.ui().set_app_theme(core.user_settings().ui.theme);
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
