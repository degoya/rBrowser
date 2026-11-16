use std::ffi::OsString;
use std::time::Duration;

use super::*;
use crate::supervisor::Sidecar;

fn settings(node: Option<&str>, browsers: Option<&str>, rpc_timeout_ms: u32) -> SidecarSettings {
    SidecarSettings {
        node_path: node.map(str::to_owned),
        rpc_timeout_ms,
        browsers_path: browsers.map(str::to_owned),
        lighthouse_browser_path: None,
    }
}

fn launcher() -> ConfiguredLauncher {
    ConfiguredLauncher::new(
        PathBuf::from("/opt/rbrowser"),
        Some("/home/user/AppData/Local".into()),
        SidecarSettings::default(),
    )
}

/// An absolute browsers folder on the platform the test runs on.
#[cfg(windows)]
const BROWSERS: &str = "C:\\pw";
#[cfg(not(windows))]
const BROWSERS: &str = "/pw";

#[test]
fn the_launcher_reads_the_settings_in_effect_at_each_start() {
    let launcher = launcher();
    let first = launcher.node_launcher().expect("defaults");
    assert_eq!(first.install_dir, PathBuf::from("/opt/rbrowser"));
    assert_eq!(first.node_path, None);
    assert_eq!(
        first.browsers_path,
        PathBuf::from("/home/user/AppData/Local/rBrowser/ms-playwright")
    );
    launcher.set_settings(settings(Some("/node/bin/node"), Some(BROWSERS), 5_000));
    let second = launcher.node_launcher().expect("changed");
    assert_eq!(second.node_path.as_deref(), Some("/node/bin/node"));
    assert_eq!(second.browsers_path, PathBuf::from(BROWSERS));
}

#[test]
fn a_bad_browsers_folder_fails_the_start_not_the_setup() {
    let launcher = launcher();
    launcher.set_settings(settings(None, Some("relative/pw"), 5_000));
    let error = launcher.node_launcher().expect_err("relative");
    assert!(matches!(error, SidecarError::BrowsersPath(_)), "{error}");
    launcher.set_settings(SidecarSettings::default());
    launcher.node_launcher().expect("fixed");
}

#[test]
fn applied_settings_reach_the_launcher_and_the_request_timeout() {
    let sidecar = Sidecar::new(launcher(), Duration::from_secs(30));
    assert_eq!(sidecar.rpc_timeout(), Duration::from_secs(30));
    sidecar.apply_settings(settings(Some("/n/node"), None, 5_000));
    assert_eq!(sidecar.rpc_timeout(), Duration::from_secs(5));
    let node = sidecar.launcher().node_launcher().expect("valid");
    assert_eq!(node.node_path.as_deref(), Some("/n/node"));
}

#[test]
fn node_gets_only_the_allowed_environment() {
    let parent = [
        ("Path", "C:\\Windows"),
        ("SystemRoot", "C:\\Windows"),
        ("TEMP", "C:\\Temp"),
        ("tmp", "C:\\Temp"),
        ("LOCALAPPDATA", "C:\\Users\\a\\AppData\\Local"),
        ("USERPROFILE", "C:\\Users\\a"),
        ("HOME", "/home/a"),
        ("ProgramFiles", "C:\\Program Files"),
        ("ProgramFiles(x86)", "C:\\Program Files (x86)"),
        ("ProgramW6432", "C:\\Program Files"),
        ("SystemDrive", "C:"),
        ("CHROME_PATH", "C:\\evil.exe"),
        ("NODE_OPTIONS", "--require evil.js"),
        ("NODE_TLS_REJECT_UNAUTHORIZED", "0"),
        ("NODE_EXTRA_CA_CERTS", "C:\\evil.pem"),
        ("NODE_PATH", "C:\\evil"),
        ("node_options", "--require evil.js"),
        ("PLAYWRIGHT_DOWNLOAD_HOST", "https://evil.example"),
        ("PLAYWRIGHT_BROWSERS_PATH", "C:\\elsewhere"),
        ("HTTPS_PROXY", "http://proxy:8080"),
        ("http_proxy", "http://proxy:8080"),
        ("RBROWSER_MCP_TOKEN", "secret"),
    ]
    .map(|(name, value)| (OsString::from(name), OsString::from(value)));
    let names: Vec<String> = allowed_env(parent)
        .into_iter()
        .map(|(name, _)| name.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "Path",
            "SystemRoot",
            "TEMP",
            "tmp",
            "LOCALAPPDATA",
            "USERPROFILE",
            "HOME",
            "ProgramFiles",
            "ProgramFiles(x86)",
            "ProgramW6432",
            "SystemDrive"
        ]
    );
}

/// RB-080-58: Lighthouse's browser search (sidecar, `lighthouse/browser.ts`) reads the program
/// folders from the environment; without them it found no machine-wide Chrome or Edge.
#[test]
fn node_gets_the_program_folders_lighthouse_looks_for_a_browser_in() {
    for name in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
        let kept = allowed_env([(OsString::from(name), OsString::from("C:\\Program Files"))]);
        assert_eq!(kept.len(), 1, "{name}");
    }
}

#[test]
fn the_lighthouse_browser_is_an_absolute_path_or_left_out() {
    let absolute = if cfg!(windows) {
        "C:\\Tools\\chrome.exe"
    } else {
        "/opt/chrome/chrome"
    };
    assert_eq!(
        lighthouse_browser(Some(&format!("  {absolute} "))),
        Some(PathBuf::from(absolute))
    );
    assert_eq!(lighthouse_browser(Some("chrome.exe")), None);
    assert_eq!(lighthouse_browser(Some("  ")), None);
    assert_eq!(lighthouse_browser(None), None);
}

/// The setting is the only way a `CHROME_PATH` reaches the sidecar: the parent's is dropped.
#[test]
fn the_sidecar_gets_chrome_path_from_the_setting_only() {
    let parent = [("CHROME_PATH", "/evil"), ("PATH", "/bin")]
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
    let value = |env: &[(OsString, OsString)], name: &str| {
        env.iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let without = sidecar_env(parent.clone(), Path::new("/pw"), None);
    assert_eq!(value(&without, "CHROME_PATH"), None);
    assert_eq!(
        value(&without, "PLAYWRIGHT_BROWSERS_PATH"),
        Some("/pw".into())
    );
    assert_eq!(value(&without, "PATH"), Some("/bin".into()));
    let with = sidecar_env(parent, Path::new("/pw"), Some(Path::new("/opt/chrome")));
    assert_eq!(value(&with, "CHROME_PATH"), Some("/opt/chrome".into()));
}

#[test]
fn the_launcher_hands_the_lighthouse_browser_to_the_next_start() {
    let launcher = launcher();
    assert_eq!(
        launcher
            .node_launcher()
            .expect("defaults")
            .lighthouse_browser,
        None
    );
    let absolute = if cfg!(windows) {
        "C:\\chrome.exe"
    } else {
        "/chrome"
    };
    launcher.set_settings(SidecarSettings {
        lighthouse_browser_path: Some(absolute.to_owned()),
        ..SidecarSettings::default()
    });
    assert_eq!(
        launcher.node_launcher().expect("set").lighthouse_browser,
        Some(PathBuf::from(absolute))
    );
}

/// Review 9: no job outside Windows is no failure; a job that could not be made on Windows is
/// a warning, and the sidecar runs on without it.
#[test]
fn a_missing_job_is_no_job() {
    let unsupported = io::Error::new(io::ErrorKind::Unsupported, "no job objects");
    assert!(tied(Err(unsupported)).is_none());
    assert!(tied(Err(io::Error::other("access denied"))).is_none());
}
