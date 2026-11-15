use super::*;

#[test]
fn the_first_line_of_an_error_is_kept_short() {
    assert_eq!(first_line("\n  \n  first\nsecond"), "first");
    assert_eq!(first_line(&"x".repeat(500)).len(), DETAIL_CHARS);
    assert_eq!(first_line(""), "");
}

#[tokio::test]
async fn a_missing_program_is_wsl_not_installed() {
    let command = wsl_command("/nonexistent/rbrowser-test/wsl.exe", &["--list"]);
    assert_eq!(
        capture(command, QUERY_TIMEOUT).await,
        Err(WslError::NotInstalled)
    );
    assert_eq!(
        list_distros("/nonexistent/rbrowser-test/wsl.exe", QUERY_TIMEOUT).await,
        Err(WslError::NotInstalled)
    );
}

#[tokio::test]
async fn invalid_distribution_names_never_reach_wsl_exe() {
    let result = wslinfo_mode("/nonexistent/wsl.exe", "a/b", QUERY_TIMEOUT).await;
    assert_eq!(
        result,
        Err(WslError::Path(PathError::InvalidDistro("a/b".into())))
    );
}

#[tokio::test]
async fn an_override_needs_no_wsl_and_is_not_cached() {
    let cache = ModeCache::default();
    let detection = detect_mode(
        "/nonexistent/wsl.exe",
        "Ubuntu",
        Some(NetworkingMode::Mirrored),
        None,
        &cache,
        QUERY_TIMEOUT,
    )
    .await;
    assert_eq!(detection.mode, NetworkingMode::Mirrored);
    assert_eq!(cache.get("Ubuntu"), None);
}

#[tokio::test]
async fn without_wslinfo_the_wslconfig_decides_and_is_cached() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config = dir.path().join(".wslconfig");
    std::fs::write(&config, "[wsl2]\nnetworkingMode=mirrored\n").expect("write");
    let cache = ModeCache::default();
    let detection = detect_mode(
        "/nonexistent/wsl.exe",
        "Ubuntu",
        None,
        Some(&config),
        &cache,
        QUERY_TIMEOUT,
    )
    .await;
    assert_eq!(
        detection,
        resolve(None, None, Some(NetworkingMode::Mirrored))
    );
    assert_eq!(cache.get("ubuntu"), Some(detection));
    let missing = dir.path().join("none");
    let other = detect_mode(
        "/nonexistent/wsl.exe",
        "Debian",
        None,
        Some(&missing),
        &cache,
        QUERY_TIMEOUT,
    )
    .await;
    assert_eq!(other, resolve(None, None, None));
}

#[cfg(unix)]
mod with_a_stand_in {
    use super::*;

    /// A shell script standing in for `wsl.exe`.
    fn stand_in(dir: &Path, body: &str) -> String {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("wsl.exe");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path.to_str().expect("utf-8 temp path").to_owned()
    }

    #[tokio::test]
    async fn the_list_and_the_running_states() {
        let dir = tempfile::tempdir().expect("temp dir");
        let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        let wsl = stand_in(
            dir.path(),
            &format!(
                "case \"$2\" in --verbose) cat {fixtures}/wsl-l-v.en.utf16le;; \
                 *) printf 'Debian\\n';; esac"
            ),
        );
        let distros = list_distros(&wsl, QUERY_TIMEOUT).await.expect("listed");
        let running: Vec<&str> = distros
            .iter()
            .filter(|d| d.state == crate::distro::DistroState::Running)
            .map(|d| d.name.as_str())
            .collect();
        assert_eq!(distros.len(), 4);
        assert_eq!(running, vec!["Debian"]);
    }

    #[tokio::test]
    async fn an_empty_list_and_a_failing_wsl() {
        let dir = tempfile::tempdir().expect("temp dir");
        let empty = stand_in(dir.path(), "exit 0");
        assert_eq!(
            list_distros(&empty, QUERY_TIMEOUT).await,
            Err(WslError::NoDistro)
        );
        let failing = stand_in(dir.path(), "echo 'WSL is off' >&2; exit 1");
        let result = capture(wsl_command(&failing, &[]), QUERY_TIMEOUT).await;
        assert!(
            matches!(&result, Err(WslError::Failed { detail, .. }) if detail == "WSL is off"),
            "{result:?}"
        );
        assert_eq!(
            list_distros(&failing, QUERY_TIMEOUT).await,
            Err(WslError::NotInstalled)
        );
    }

    #[tokio::test]
    async fn a_hanging_wsl_is_stopped_at_the_limit() {
        let dir = tempfile::tempdir().expect("temp dir");
        let hanging = stand_in(dir.path(), "sleep 30");
        let limit = Duration::from_millis(200);
        assert_eq!(
            capture(wsl_command(&hanging, &[]), limit).await,
            Err(WslError::Timeout(200))
        );
    }

    #[tokio::test]
    async fn wslinfo_names_the_mode_and_wins_over_the_wslconfig() {
        let dir = tempfile::tempdir().expect("temp dir");
        let wsl = stand_in(
            dir.path(),
            "[ \"$1 $2 $3 $4 $5\" = \"-d Ubuntu -e wslinfo --networking-mode\" ] && echo nat",
        );
        assert_eq!(
            wslinfo_mode(&wsl, "Ubuntu", QUERY_TIMEOUT).await,
            Ok(Some(NetworkingMode::Nat))
        );
        let config = dir.path().join(".wslconfig");
        std::fs::write(&config, "[wsl2]\nnetworkingMode=mirrored\n").expect("write");
        let cache = ModeCache::default();
        let detection =
            detect_mode(&wsl, "Ubuntu", None, Some(&config), &cache, QUERY_TIMEOUT).await;
        assert_eq!(detection, resolve(None, Some(NetworkingMode::Nat), None));
    }
}
