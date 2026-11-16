use super::*;

#[test]
fn a_linux_path_from_wsl_becomes_the_wsl_share() {
    assert_eq!(
        translate("/home/u/app", true, Some("Ubuntu")).expect("translates"),
        "\\\\wsl.localhost\\Ubuntu\\home\\u\\app"
    );
    assert_eq!(
        translate("/home/u/app/", true, Some("Ubuntu-24.04")).expect("translates"),
        "\\\\wsl.localhost\\Ubuntu-24.04\\home\\u\\app"
    );
}

#[test]
fn a_windows_drive_under_mnt_becomes_the_drive() {
    assert_eq!(
        translate("/mnt/c/work/site", true, Some("Ubuntu")).expect("translates"),
        "C:\\work\\site"
    );
    assert_eq!(translate("/mnt/d", true, None).expect("translates"), "D:\\");
}

#[test]
fn a_linux_path_without_a_distribution_is_refused() {
    assert_eq!(
        translate("/home/u/app", true, None),
        Err(PathError::NoDistro)
    );
}

#[test]
fn windows_paths_and_other_platforms_stay_as_written() {
    for path in [
        "C:\\site",
        "\\\\wsl.localhost\\Ubuntu\\home\\u",
        "relative\\dir",
    ] {
        assert_eq!(translate(path, true, Some("Ubuntu")).as_deref(), Ok(path));
    }
    assert_eq!(
        translate("/home/u/app", false, None).as_deref(),
        Ok("/home/u/app")
    );
}

#[test]
fn the_distribution_comes_from_the_working_folder_on_the_share() {
    assert_eq!(
        distro_from_cwd("\\\\wsl.localhost\\Ubuntu\\home\\u").as_deref(),
        Some("Ubuntu")
    );
    assert_eq!(
        distro_from_cwd("\\\\wsl$\\Debian\\tmp").as_deref(),
        Some("Debian")
    );
    assert_eq!(
        distro_from_cwd("\\\\?\\UNC\\wsl.localhost\\Arch\\").as_deref(),
        Some("Arch")
    );
    assert_eq!(distro_from_cwd("C:\\Users\\u"), None);
    assert_eq!(distro_from_cwd("\\\\wsl.localhost\\bad name\\x"), None);
}

#[test]
fn a_distribution_name_is_checked() {
    assert!(valid_distro("Ubuntu-24.04"));
    assert!(valid_distro("my_distro"));
    assert!(!valid_distro(""));
    assert!(!valid_distro("a\\b"));
    assert!(!valid_distro("..\\x"));
    assert!(!valid_distro(&"x".repeat(65)));
}

#[test]
fn paths_are_told_from_urls() {
    for path in [
        ".",
        "..\\site",
        "./app",
        "/home/u",
        "\\\\server\\share",
        "C:\\x",
        "d:/x",
        "C:",
    ] {
        assert!(looks_like_path(path), "{path}");
    }
    for url in [
        "http://localhost:5173/a",
        "localhost:5173",
        "example.com",
        "about:blank",
        "file:///C:/x.html",
    ] {
        assert!(!looks_like_path(url), "{url}");
    }
}

#[test]
fn a_relative_folder_is_made_absolute_against_the_working_folder() {
    let cwd = std::env::temp_dir().join("rb-cwd");
    assert_eq!(absolute(Path::new("app"), &cwd), cwd.join("app"));
    assert_eq!(absolute(Path::new("./app/../site"), &cwd), cwd.join("site"));
    assert_eq!(absolute(Path::new("."), &cwd), cwd);
    let elsewhere = std::env::temp_dir().join("x");
    assert_eq!(absolute(&elsewhere, &cwd), elsewhere);
}
