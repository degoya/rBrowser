use super::*;

fn ubuntu() -> Translator {
    Translator::new("Ubuntu").expect("a valid name")
}

#[test]
fn windows_to_linux_cases() {
    let t = ubuntu();
    let cases: &[(&str, &str)] = &[
        (r"\\wsl.localhost\Ubuntu\home\u\app", "/home/u/app"),
        (r"\\wsl$\Ubuntu\home\u\app", "/home/u/app"),
        (r"\\WSL.LOCALHOST\ubuntu\home\u\app", "/home/u/app"),
        (r"\\wsl$\UBUNTU\srv", "/srv"),
        (r"\\wsl.localhost\Ubuntu", "/"),
        (r"\\wsl.localhost\Ubuntu\", "/"),
        (r"\\wsl.localhost\Ubuntu\home\u\app\", "/home/u/app"),
        (r"\\?\UNC\wsl.localhost\Ubuntu\home\u", "/home/u"),
        ("//wsl.localhost/Ubuntu/home/u", "/home/u"),
        (r"\\wsl.localhost\Ubuntu\home\u\my app", "/home/u/my app"),
        (
            "\\\\wsl.localhost\\Ubuntu\\home\\j\u{fc}rgen\\\u{dc}bung",
            "/home/j\u{fc}rgen/\u{dc}bung",
        ),
        (r"\\wsl.localhost\Ubuntu\home\u\.\app", "/home/u/app"),
        (r"\\wsl.localhost\Ubuntu\home\u\a#b (1)", "/home/u/a#b (1)"),
        (r"C:\Users\Me\site", "/mnt/c/Users/Me/site"),
        (r"c:\Users\Me", "/mnt/c/Users/Me"),
        ("D:/work/My Site", "/mnt/d/work/My Site"),
        (r"C:\", "/mnt/c"),
        ("C:", "/mnt/c"),
        (r"\\?\C:\long\path", "/mnt/c/long/path"),
        ("E:\\\u{c4}rger\\na\u{ef}ve", "/mnt/e/\u{c4}rger/na\u{ef}ve"),
    ];
    for (windows, linux) in cases {
        assert_eq!(t.to_linux(windows).as_deref(), Ok(*linux), "{windows}");
    }
}

#[test]
fn linux_to_windows_cases() {
    let t = ubuntu();
    let cases: &[(&str, &str)] = &[
        ("/home/u/app", r"\\wsl.localhost\Ubuntu\home\u\app"),
        ("/", r"\\wsl.localhost\Ubuntu\"),
        ("/home/u/app/", r"\\wsl.localhost\Ubuntu\home\u\app"),
        ("//home///u", r"\\wsl.localhost\Ubuntu\home\u"),
        ("/home/u/my app", r"\\wsl.localhost\Ubuntu\home\u\my app"),
        (
            "/home/j\u{fc}rgen/\u{dc}bung",
            "\\\\wsl.localhost\\Ubuntu\\home\\j\u{fc}rgen\\\u{dc}bung",
        ),
        ("/mnt/c/Users/Me", r"C:\Users\Me"),
        ("/mnt/c", r"C:\"),
        ("/mnt/c/", r"C:\"),
        ("/mnt/d/My Site/\u{e4}", "D:\\My Site\\\u{e4}"),
        ("/mnt/wsl/shared", r"\\wsl.localhost\Ubuntu\mnt\wsl\shared"),
        ("/mnt/cd/x", r"\\wsl.localhost\Ubuntu\mnt\cd\x"),
        ("/mnt/C/x", r"\\wsl.localhost\Ubuntu\mnt\C\x"),
        ("/mnt", r"\\wsl.localhost\Ubuntu\mnt"),
    ];
    for (linux, windows) in cases {
        assert_eq!(t.to_windows(linux).as_deref(), Ok(*windows), "{linux}");
    }
}

#[test]
fn round_trips_hold() {
    let t = ubuntu();
    for linux in [
        "/home/u/app",
        "/mnt/c/Users/Me",
        "/opt/a b/\u{fc}",
        "/",
        "/mnt/z",
    ] {
        let windows = t.to_windows(linux).expect("translates");
        assert_eq!(t.to_linux(&windows).as_deref(), Ok(linux), "{windows}");
    }
}

#[test]
fn the_legacy_host_and_another_mount_root() {
    let t = ubuntu().with_host(UncHost::Legacy).with_mount_root("/");
    assert_eq!(
        t.to_windows("/home/u").as_deref(),
        Ok(r"\\wsl$\Ubuntu\home\u")
    );
    assert_eq!(t.to_windows("/c/Users").as_deref(), Ok(r"C:\Users"));
    assert_eq!(t.to_linux(r"C:\Users").as_deref(), Ok("/c/Users"));
    let t = ubuntu().with_mount_root("/windows/");
    assert_eq!(t.to_linux(r"C:\x").as_deref(), Ok("/windows/c/x"));
    assert_eq!(t.to_windows("/windows/c/x").as_deref(), Ok(r"C:\x"));
    assert_eq!(
        t.to_windows("/mnt/c/x").as_deref(),
        Ok(r"\\wsl.localhost\Ubuntu\mnt\c\x")
    );
}

#[test]
fn a_distribution_name_with_dots_and_dashes() {
    let t = Translator::new("Ubuntu-24.04").expect("valid");
    assert_eq!(
        t.to_windows("/srv").as_deref(),
        Ok(r"\\wsl.localhost\Ubuntu-24.04\srv")
    );
    assert_eq!(
        t.to_linux(r"\\wsl$\ubuntu-24.04\srv").as_deref(),
        Ok("/srv")
    );
}

#[test]
fn what_has_no_path_on_the_other_side() {
    let t = ubuntu();
    assert_eq!(
        t.to_linux(r"\\wsl.localhost\Debian\home"),
        Err(PathError::OtherDistro {
            expected: "Ubuntu".into(),
            found: "Debian".into()
        })
    );
    assert_eq!(
        t.to_linux(r"\\server\share\x"),
        Err(PathError::NetworkShare)
    );
    assert_eq!(
        t.to_linux(r"\\?\UNC\server\share"),
        Err(PathError::NetworkShare)
    );
    assert_eq!(t.to_linux(r"C:relative"), Err(PathError::NotAbsolute));
    assert_eq!(t.to_linux(r"relative\x"), Err(PathError::NotAbsolute));
    assert_eq!(t.to_linux(""), Err(PathError::NotAbsolute));
    assert_eq!(t.to_windows("home/u"), Err(PathError::NotAbsolute));
    assert_eq!(t.to_windows(""), Err(PathError::NotAbsolute));
    assert_eq!(t.to_windows(r"/home/a\b"), Err(PathError::Backslash));
}

#[test]
fn parent_components_are_refused_both_ways() {
    let t = ubuntu();
    assert_eq!(
        t.to_linux(r"\\wsl.localhost\Ubuntu\home\..\etc"),
        Err(PathError::ParentComponent)
    );
    assert_eq!(t.to_linux(r"C:\a\..\b"), Err(PathError::ParentComponent));
    assert_eq!(
        t.to_windows("/home/../etc"),
        Err(PathError::ParentComponent)
    );
    assert_eq!(t.to_windows("/mnt/c/.."), Err(PathError::ParentComponent));
}

#[test]
fn parse_unc_names_the_distribution() {
    assert_eq!(
        parse_unc(r"\\wsl$\Ubuntu-22.04\home\u"),
        Ok(WslPath {
            distro: "Ubuntu-22.04".into(),
            linux_path: "/home/u".into()
        })
    );
    assert_eq!(parse_unc(r"C:\x"), Err(PathError::NotWsl));
    assert_eq!(
        parse_unc(r"\\wsl.localhost\"),
        Err(PathError::InvalidDistro(String::new()))
    );
    assert_eq!(
        parse_unc(r"\\wsl.localhost\a:b\x"),
        Err(PathError::InvalidDistro("a:b".into()))
    );
    assert!(is_wsl_unc(r"\\WSL$\x"));
    assert!(!is_wsl_unc(r"\\wslhost\x"));
}

#[test]
fn unc_path_and_its_refusals() {
    assert_eq!(
        unc_path("Ubuntu", "/home/u", UncHost::Localhost).as_deref(),
        Ok(r"\\wsl.localhost\Ubuntu\home\u")
    );
    assert_eq!(
        unc_path("Bad/Name", "/", UncHost::Localhost),
        Err(PathError::InvalidDistro("Bad/Name".into()))
    );
    assert!(Translator::new("..").is_err());
    assert!(Translator::new("").is_err());
    assert!(Translator::new("a*b").is_err());
}

#[test]
fn every_error_has_a_key_under_wsl_error_path() {
    let errors = [
        PathError::NotWsl,
        PathError::InvalidDistro("x".into()),
        PathError::NotAbsolute,
        PathError::ParentComponent,
        PathError::Backslash,
        PathError::NetworkShare,
        PathError::OtherDistro {
            expected: "a".into(),
            found: "b".into(),
        },
    ];
    for error in errors {
        assert!(error.i18n_key().starts_with("wsl.error.path."), "{error}");
    }
}

#[test]
fn normalized_linux_paths() {
    assert_eq!(
        normalize_linux("//opt/./rb//bin/").as_deref(),
        Ok("/opt/rb/bin")
    );
    assert_eq!(normalize_linux("/").as_deref(), Ok("/"));
    assert_eq!(normalize_linux("opt"), Err(PathError::NotAbsolute));
    assert_eq!(
        normalize_linux("/opt/../etc"),
        Err(PathError::ParentComponent)
    );
}
