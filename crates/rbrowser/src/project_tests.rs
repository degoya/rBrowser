use super::*;

const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/grid-project"
);

#[test]
fn the_root_is_found_from_a_subfolder() {
    let nested = Path::new(FIXTURE).join(".rbrowser");
    let root = find_root(None, &nested).expect("found");
    assert!(root.join(CONFIG_DIR).join(CONFIG_FILE).is_file());
    let given = Path::new("C:\\work\\site");
    assert_eq!(find_root(Some(given), &nested), Some(given.to_path_buf()));
    let empty = tempfile::tempdir().expect("temp dir");
    assert_eq!(find_root(None, empty.path()), None);
}

#[test]
fn the_fixture_config_loads() {
    let config = load(Path::new(FIXTURE)).expect("loads");
    assert_eq!(config.layout.cells.len(), 6);
    let missing = tempfile::tempdir().expect("temp dir");
    assert!(load(missing.path()).is_err());
}

#[test]
fn a_size_from_an_unknown_device_preset_refuses_the_config() {
    let devices = rb_config::builtin_devices().expect("the built-in list");
    let config = |device: &str| -> ProjectConfig {
        serde_json::from_value(serde_json::json!({
            "version": 1,
            "breakpoints": [{ "id": "phone", "device": device }]
        }))
        .expect("a config")
    };
    assert!(check_sizes(&config("iphone-15"), &devices).is_ok());
    let Err(ConfigError::Invalid(issues)) = check_sizes(&config("no-such-phone"), &devices) else {
        panic!("expected issues");
    };
    assert_eq!(issues[0].path, "/breakpoints/0/device");
    assert_eq!(issues[0].key, "config.error.unknown_device");
}

#[test]
fn each_project_has_its_own_profile() {
    let base = Path::new("C:\\Users\\a\\AppData\\Local\\rBrowser");
    let one = profile_dir(base, Some(Path::new("C:\\work\\site")));
    let same = profile_dir(base, Some(Path::new("c:/WORK/site/")));
    let other = profile_dir(base, Some(Path::new("D:\\work\\site")));
    assert_eq!(one, same, "the same folder, written differently");
    assert_ne!(one, other, "equal folder names stay apart");
    let name = one
        .file_name()
        .and_then(|name| name.to_str())
        .expect("name");
    assert!(name.starts_with("site-") && name.len() == 5 + 16, "{name}");
    assert_eq!(
        profile_dir(base, None),
        base.join("profiles").join("default")
    );
    let odd = profile_dir(
        base,
        Some(Path::new("\\\\wsl.localhost\\Debian\\home\\a\\my site!")),
    );
    let odd = odd
        .file_name()
        .and_then(|name| name.to_str())
        .expect("name");
    assert!(odd.starts_with("my_site_-"), "{odd}");
}

#[test]
fn the_wsl_share_is_recognised() {
    assert!(is_wsl_share(Path::new(
        "\\\\wsl.localhost\\Debian\\home\\a\\rbrowser.exe"
    )));
    assert!(is_wsl_share(Path::new("\\\\WSL$\\Debian\\x")));
    assert!(is_wsl_share(Path::new(
        "\\\\?\\UNC\\wsl.localhost\\Debian\\x"
    )));
    assert!(!is_wsl_share(Path::new(
        "C:\\Users\\a\\AppData\\Local\\rbrowser-dev\\rbrowser.exe"
    )));
    assert!(!is_wsl_share(Path::new("\\\\server\\share\\rbrowser.exe")));
}
