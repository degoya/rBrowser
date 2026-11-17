use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::*;

fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let vars: HashMap<String, OsString> = vars
        .iter()
        .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
        .collect();
    move |name| vars.get(name).cloned()
}

#[test]
fn the_candidates_are_the_installed_search_order_as_paths() {
    let found = candidates(
        true,
        Some(Candidate {
            path: PathBuf::from(r"D:\Apps\Firefox\firefox.exe"),
            kind: Kind::Release,
        }),
        Vec::new(),
        env(&[("ProgramFiles", r"C:\Program Files")]),
    );
    assert_eq!(found[0], PathBuf::from(r"D:\Apps\Firefox\firefox.exe"));
    assert_eq!(
        found[1],
        Path::new(r"C:\Program Files")
            .join("Mozilla Firefox")
            .join("firefox.exe")
    );
    assert_eq!(found.len(), 5, "{found:?}");
    let found = candidates(false, None, Vec::new(), env(&[]));
    assert!(
        found.contains(&PathBuf::from("/usr/bin/firefox")),
        "{found:?}"
    );
}

#[test]
fn the_setting_wins_and_must_exist() {
    let exists = |path: &Path| path == Path::new("/opt/ff/firefox") || path == Path::new("/b");
    assert_eq!(
        locate(
            Some(Path::new("/opt/ff/firefox")),
            vec![PathBuf::from("/b")],
            exists
        ),
        Ok(PathBuf::from("/opt/ff/firefox"))
    );
    assert_eq!(
        locate(
            Some(Path::new("/missing")),
            vec![PathBuf::from("/b")],
            exists
        ),
        Err(LocateError::Configured(PathBuf::from("/missing"))),
        "a wrong setting is reported, not papered over"
    );
    assert_eq!(
        locate(None, vec![PathBuf::from("/a"), PathBuf::from("/b")], exists),
        Ok(PathBuf::from("/b"))
    );
    assert_eq!(
        locate(None, vec![PathBuf::from("/a")], exists),
        Err(LocateError::NotFound)
    );
    assert_eq!(
        locate(Some(Path::new("")), vec![PathBuf::from("/b")], exists),
        Ok(PathBuf::from("/b")),
        "an empty setting means automatic"
    );
}

#[test]
fn the_endpoint_comes_from_the_listening_line() {
    assert_eq!(
        endpoint_in("WebDriver BiDi listening on ws://127.0.0.1:53817"),
        Some("ws://127.0.0.1:53817/session".to_owned())
    );
    assert_eq!(
        endpoint_in(
            "1730000000000\tRemoteAgent\tINFO\tWebDriver BiDi listening on ws://127.0.0.1:9222/ \r"
        ),
        Some("ws://127.0.0.1:9222/session".to_owned())
    );
    assert_eq!(endpoint_in("Read port: 0"), None);
    assert_eq!(endpoint_in("WebDriver BiDi listening on "), None);
}

#[test]
fn firefox_starts_with_its_own_profile_on_a_random_port() {
    let args = arguments(Path::new("/p/ff-profile"));
    let args: Vec<String> = args
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        args,
        [
            "-profile",
            "/p/ff-profile",
            "-no-remote",
            "-new-instance",
            "-wait-for-browser",
            "--remote-debugging-port",
            "0",
            "about:blank",
        ]
    );
}

#[tokio::test]
async fn a_missing_executable_is_a_launch_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("no-firefox.exe");
    let error = launch(&missing, dir.path(), std::time::Duration::from_secs(1))
        .await
        .expect_err("nothing to start");
    assert!(matches!(error, crate::BidiError::Launch(_)), "{error}");
}

/// A Firefox that quit by itself (exit status 0: it quits with its last window) ended cleanly;
/// one that ended with an error did not (RB-080-38).
#[cfg(unix)]
#[tokio::test]
async fn a_clean_exit_is_told_from_a_crash() {
    let spawn = |ok: bool| {
        let child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(if ok { "exit 0" } else { "exit 3" })
            .kill_on_drop(true)
            .spawn()
            .expect("sh runs");
        FirefoxProcess {
            child,
            endpoint: "ws://127.0.0.1:1/session".to_owned(),
        }
    };
    let grace = std::time::Duration::from_secs(10);
    assert_eq!(spawn(true).ended_cleanly(grace).await, Some(true));
    assert_eq!(spawn(false).ended_cleanly(grace).await, Some(false));
}
