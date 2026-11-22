use std::path::{Path, PathBuf};

use super::*;

#[test]
fn gtk_theme_names_a_dark_variant() {
    assert!(gtk_theme_is_dark("Adwaita:dark"));
    assert!(gtk_theme_is_dark("Adwaita-dark"));
    assert!(gtk_theme_is_dark("Yaru-Dark"));
    assert!(gtk_theme_is_dark(" Breeze-Dark "));
    assert!(!gtk_theme_is_dark("Adwaita"));
    assert!(!gtk_theme_is_dark("Adwaita:light"));
    assert!(!gtk_theme_is_dark("Darkly-light"));
    assert!(!gtk_theme_is_dark(""));
}

#[test]
fn the_gtk_settings_file_prefers_dark_or_names_a_dark_theme() {
    let prefer = "[Settings]\ngtk-application-prefer-dark-theme=1\n";
    assert!(settings_ini_is_dark(prefer));
    assert!(settings_ini_is_dark(
        "[Settings]\ngtk-application-prefer-dark-theme = true\n"
    ));
    assert!(settings_ini_is_dark(
        "[Settings]\ngtk-theme-name=Adwaita-dark\n"
    ));
    assert!(!settings_ini_is_dark(
        "[Settings]\ngtk-application-prefer-dark-theme=0\n"
    ));
    assert!(!settings_ini_is_dark(
        "[Settings]\ngtk-theme-name=Adwaita\n"
    ));
    // A comment or another section's key is not the setting.
    assert!(!settings_ini_is_dark(
        "# gtk-application-prefer-dark-theme=1\n"
    ));
    assert!(!settings_ini_is_dark(
        "[Other]\ngtk-application-prefer-dark-theme=1\n"
    ));
    assert!(!settings_ini_is_dark(""));
}

#[test]
fn the_environment_wins_over_the_settings_file() {
    let file = Some("[Settings]\ngtk-application-prefer-dark-theme=1\n");
    assert!(!system_dark_from(Some("Adwaita"), file));
    assert!(system_dark_from(Some("Adwaita:dark"), None));
    assert!(system_dark_from(None, file));
    assert!(system_dark_from(Some(""), file));
    assert!(!system_dark_from(None, None));
}

#[test]
fn the_settings_file_lies_under_the_xdg_config_home() {
    let var = |name: &str| match name {
        "XDG_CONFIG_HOME" => Some("/x/config".into()),
        "HOME" => Some("/home/me".into()),
        _ => None,
    };
    assert_eq!(
        gtk_settings_path(var),
        Some(PathBuf::from("/x/config/gtk-3.0/settings.ini"))
    );
    let home_only = |name: &str| (name == "HOME").then(|| "/home/me".into());
    assert_eq!(
        gtk_settings_path(home_only),
        Some(Path::new("/home/me/.config/gtk-3.0/settings.ini").to_path_buf())
    );
    assert_eq!(gtk_settings_path(|_| None), None);
}
