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
fn windows_uses_local_app_data() {
    let paths = DiagPaths::resolve(true, env(&[("LOCALAPPDATA", "/c/Users/a/AppData/Local")]))
        .expect("paths");
    let root = Path::new("/c/Users/a/AppData/Local/rBrowser");
    assert_eq!(paths.logs, root.join("logs"));
    assert_eq!(paths.log_file(), root.join("logs").join("rbrowser.log"));
    assert_eq!(paths.cef_log_file(), root.join("logs").join("cef.log"));
    assert_eq!(paths.crash_dumps, root.join("User Data").join("Crashpad"));
    assert_eq!(paths.bundles, root.join("diagnostics"));
}

#[test]
fn windows_without_local_app_data_has_no_paths() {
    assert_eq!(DiagPaths::resolve(true, env(&[("HOME", "/home/a")])), None);
    assert_eq!(DiagPaths::resolve(true, env(&[("LOCALAPPDATA", "")])), None);
}

#[test]
fn elsewhere_the_state_directory_is_the_fallback() {
    let xdg = DiagPaths::resolve(false, env(&[("XDG_STATE_HOME", "/s"), ("HOME", "/home/a")]))
        .expect("paths");
    assert_eq!(xdg.logs, Path::new("/s/rBrowser/logs"));
    let home = DiagPaths::resolve(false, env(&[("HOME", "/home/a")])).expect("paths");
    assert_eq!(home.logs, Path::new("/home/a/.local/state/rBrowser/logs"));
    assert_eq!(DiagPaths::resolve(false, env(&[])), None);
}
