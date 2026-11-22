//! What the Linux desktop says of the app's theme (RB-097-05): whether it prefers a dark one,
//! which resolves the theme `system` (`cef_app/linux/theme_dwm.rs`). Read without starting a
//! program, from what GTK itself reads: `GTK_THEME` (`Adwaita:dark`, `Yaru-dark`) first, then
//! `gtk-application-prefer-dark-theme` and `gtk-theme-name` in `$XDG_CONFIG_HOME/gtk-3.0/
//! settings.ini`. GNOME's `color-scheme` lives in dconf, which no file read reaches: there the
//! theme `system` is light unless one of the two says dark, and `ui.theme` picks dark itself.
//!
//! Pure and portable; the one environment read is [`system_dark`].

use std::ffi::OsString;
use std::path::PathBuf;

/// Whether a GTK theme name (`GTK_THEME`'s form: `<name>[:<variant>]`) is a dark one.
#[must_use]
pub fn gtk_theme_is_dark(theme: &str) -> bool {
    let theme = theme.trim().to_ascii_lowercase();
    match theme.split_once(':') {
        Some((_, variant)) => variant == "dark",
        None => theme.ends_with("-dark"),
    }
}

/// Whether GTK's `settings.ini` text prefers dark: `gtk-application-prefer-dark-theme` true, or
/// a `gtk-theme-name` that [`gtk_theme_is_dark`], in its `[Settings]` section.
#[must_use]
pub fn settings_ini_is_dark(text: &str) -> bool {
    let mut in_settings = false;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_settings = line == "[Settings]";
            continue;
        }
        if !in_settings || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = value.trim();
        match key.trim() {
            "gtk-application-prefer-dark-theme" if matches!(value, "1" | "true") => return true,
            "gtk-theme-name" if gtk_theme_is_dark(value) => return true,
            _ => {}
        }
    }
    false
}

/// The verdict from `GTK_THEME` (an empty one counts as unset) and the settings file's text.
#[must_use]
pub fn system_dark_from(gtk_theme: Option<&str>, settings_ini: Option<&str>) -> bool {
    match gtk_theme.map(str::trim).filter(|theme| !theme.is_empty()) {
        Some(theme) => gtk_theme_is_dark(theme),
        None => settings_ini.is_some_and(settings_ini_is_dark),
    }
}

/// GTK 3's settings file: `$XDG_CONFIG_HOME/gtk-3.0/settings.ini`, else under `~/.config`.
pub fn gtk_settings_path(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |name: &str| {
        var(name)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
    };
    let config = set("XDG_CONFIG_HOME").or_else(|| set("HOME").map(|home| home.join(".config")))?;
    Some(config.join("gtk-3.0").join("settings.ini"))
}

/// Whether this desktop prefers a dark theme, read now from the environment and the file.
#[must_use]
pub fn system_dark() -> bool {
    let theme = std::env::var("GTK_THEME").ok();
    let file = gtk_settings_path(|name| std::env::var_os(name))
        .and_then(|path| std::fs::read_to_string(path).ok());
    system_dark_from(theme.as_deref(), file.as_deref())
}

#[cfg(test)]
#[path = "desktop_linux_tests.rs"]
mod tests;
