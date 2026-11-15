use super::*;

fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
    move |name| {
        pairs
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| OsString::from(value))
    }
}

#[test]
fn installed_on_windows_uses_appdata_and_localappdata() {
    let vars = env(&[
        ("APPDATA", r"C:\Users\a\AppData\Roaming"),
        ("LOCALAPPDATA", r"C:\Users\a\AppData\Local"),
    ]);
    assert_eq!(
        roaming_dir_for(true, None, &vars),
        Some(PathBuf::from(r"C:\Users\a\AppData\Roaming").join("rBrowser"))
    );
    assert_eq!(
        local_dir_for(true, None, &vars),
        Some(PathBuf::from(r"C:\Users\a\AppData\Local").join("rBrowser"))
    );
    assert_eq!(roaming_dir_for(true, None, env(&[("APPDATA", "")])), None);
}

#[test]
fn elsewhere_xdg_comes_before_home() {
    let vars = env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/home/a")]);
    assert_eq!(
        local_dir_for(false, None, &vars),
        Some(PathBuf::from("/s/rBrowser"))
    );
    assert_eq!(
        roaming_dir_for(false, None, &vars),
        Some(PathBuf::from("/home/a/.config/rbrowser"))
    );
}

#[test]
fn portable_puts_both_beside_the_executable() {
    let root = PathBuf::from(r"D:\tools\rbrowser");
    assert_eq!(
        roaming_dir_for(true, Some(root.clone()), env(&[])),
        Some(root.join("data"))
    );
    assert_eq!(
        local_dir_for(true, Some(root.clone()), env(&[])),
        Some(root.join("data").join("local"))
    );
}

#[test]
fn the_user_settings_path_is_inside_the_roaming_dir() {
    let vars = env(&[("APPDATA", r"C:\R")]);
    assert_eq!(
        roaming_dir_for(true, None, &vars).map(|dir| dir.join(crate::USER_SETTINGS_FILE)),
        crate::settings_path_for(true, &vars)
    );
}

#[test]
fn the_marker_beside_the_executable_makes_its_folder_the_portable_root() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let exe = folder.path().join("rbrowser.exe");
    assert_eq!(
        portable_root_for(Some(&exe)),
        None,
        "no marker, no portable mode"
    );

    std::fs::write(folder.path().join(PORTABLE_MARKER), b"").expect("write the marker");
    assert_eq!(
        portable_root_for(Some(&exe)),
        Some(folder.path().to_path_buf())
    );
    assert_eq!(
        local_dir_for(true, portable_root_for(Some(&exe)), env(&[])),
        Some(folder.path().join("data").join("local"))
    );
}

#[test]
fn only_a_file_named_portable_switches_the_mode() {
    let folder = tempfile::tempdir().expect("temporary folder");
    let exe = folder.path().join("rbrowser.exe");
    std::fs::create_dir(folder.path().join(PORTABLE_MARKER)).expect("a folder named portable");
    assert_eq!(portable_root_for(Some(&exe)), None);
    std::fs::write(folder.path().join("rbrowser.portable"), b"").expect("another name");
    assert_eq!(portable_root_for(Some(&exe)), None);
}

#[test]
fn no_executable_path_is_no_portable_mode() {
    assert_eq!(portable_root_for(None), None);
    assert_eq!(portable_root_for(Some(Path::new("rbrowser.exe"))), None);
}
