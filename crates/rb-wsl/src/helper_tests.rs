use super::*;

fn ubuntu() -> Translator {
    Translator::new("Ubuntu").expect("valid")
}

#[test]
fn the_packaged_helper_through_the_drive_mount() {
    assert_eq!(
        helper_linux_path(&ubuntu(), r"C:\Program Files\rBrowser", None).as_deref(),
        Ok("/mnt/c/Program Files/rBrowser/rbrowser-wsl")
    );
    assert_eq!(
        helper_linux_path(&ubuntu(), r"C:\", None).as_deref(),
        Ok("/mnt/c/rbrowser-wsl")
    );
    assert_eq!(
        helper_linux_path(&ubuntu(), r"\\server\apps\rBrowser", None),
        Err(PathError::NetworkShare)
    );
}

/// The owner's unpacked package (C:\Tools\rbrowser, the log of build 10fb32e), also as the
/// verbatim path `current_exe` can give.
#[test]
fn the_helper_beside_an_unpacked_package() {
    for exe_dir in [
        r"C:\Tools\rbrowser",
        r"c:\Tools\rbrowser\",
        r"\\?\C:\Tools\rbrowser",
    ] {
        assert_eq!(
            helper_linux_path(&ubuntu(), exe_dir, None).as_deref(),
            Ok("/mnt/c/Tools/rbrowser/rbrowser-wsl"),
            "{exe_dir}"
        );
    }
}

#[test]
fn a_configured_helper_as_linux_or_windows_path() {
    let t = ubuntu();
    assert_eq!(
        helper_linux_path(&t, r"C:\x", Some("/opt/rb//rbrowser-wsl")).as_deref(),
        Ok("/opt/rb/rbrowser-wsl")
    );
    assert_eq!(
        helper_linux_path(
            &t,
            r"C:\x",
            Some(r"\\wsl.localhost\Ubuntu\opt\rbrowser-wsl")
        )
        .as_deref(),
        Ok("/opt/rbrowser-wsl")
    );
    assert_eq!(
        helper_linux_path(&t, r"C:\x", Some("  ")).as_deref(),
        Ok("/mnt/c/x/rbrowser-wsl")
    );
    assert_eq!(
        helper_linux_path(&t, r"C:\x", Some("/opt/../bin/sh")),
        Err(PathError::ParentComponent)
    );
}

#[test]
fn the_wsl_command_line() {
    let args = helper_args(
        "Ubuntu",
        "/mnt/c/Program Files/rBrowser/rbrowser-wsl",
        &["watch".to_owned(), "/home/u/app".to_owned()],
    )
    .expect("valid");
    assert_eq!(
        args,
        [
            "-d",
            "Ubuntu",
            "-e",
            "/mnt/c/Program Files/rBrowser/rbrowser-wsl",
            "watch",
            "/home/u/app"
        ]
    );
    assert_eq!(
        helper_args("-x/y", "/h", &[]),
        Err(PathError::InvalidDistro("-x/y".into()))
    );
    assert_eq!(helper_args("Ubuntu", "h", &[]), Err(PathError::NotAbsolute));
}

#[test]
fn the_factory_builds_the_same_command_each_time() {
    let mut factory = helper_command_factory(
        r"C:\Windows\System32\wsl.exe".to_owned(),
        vec!["-d".to_owned(), "Ubuntu".to_owned()],
    );
    for _ in 0..2 {
        let command = factory();
        let args: Vec<_> = command.as_std().get_args().collect();
        assert_eq!(args, ["-d", "Ubuntu"]);
    }
}

#[test]
fn restarts_back_off_and_give_up() {
    let policy = RestartPolicy {
        max_crashes: 3,
        window: Duration::from_secs(60),
        first_delay: Duration::from_millis(100),
        max_delay: Duration::from_millis(250),
    };
    let mut tracker = RestartTracker::new(policy);
    let t0 = Instant::now();
    assert_eq!(tracker.on_crash(t0), Some(Duration::from_millis(100)));
    assert_eq!(
        tracker.on_crash(t0 + Duration::from_secs(1)),
        Some(Duration::from_millis(200))
    );
    assert_eq!(
        tracker.on_crash(t0 + Duration::from_secs(2)),
        Some(Duration::from_millis(250))
    );
    assert_eq!(tracker.on_crash(t0 + Duration::from_secs(3)), None);
    assert_eq!(tracker.recent(), 4);
    tracker.reset();
    assert_eq!(
        tracker.on_crash(t0 + Duration::from_secs(4)),
        Some(Duration::from_millis(100))
    );
}

#[test]
fn old_crashes_leave_the_window() {
    let policy = RestartPolicy {
        max_crashes: 1,
        window: Duration::from_secs(10),
        ..RestartPolicy::default()
    };
    let mut tracker = RestartTracker::new(policy);
    let t0 = Instant::now();
    assert!(tracker.on_crash(t0).is_some());
    assert!(tracker.on_crash(t0 + Duration::from_secs(11)).is_some());
    assert!(tracker.on_crash(t0 + Duration::from_secs(12)).is_none());
}
