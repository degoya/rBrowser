use std::path::Path;

use super::*;

#[test]
fn the_default_file_is_the_sessions_under_out_state() {
    assert_eq!(default_name("admin"), "state/admin.json");
    let root = Path::new("/project");
    assert_eq!(
        resolve(root, "state/admin.json").expect("inside"),
        root.join(".rbrowser")
            .join("out")
            .join("state")
            .join("admin.json")
    );
    assert_eq!(
        resolve(root, "login.json").expect("inside"),
        root.join(".rbrowser").join("out").join("login.json")
    );
}

#[test]
fn a_name_that_leaves_out_or_is_no_json_file_is_refused() {
    let root = Path::new("/project");
    for name in [
        "",
        "../config.json",
        "state/../../config.json",
        "/etc/passwd.json",
        "C:/Users/x/state.json",
        "C:\\Users\\x\\state.json",
        "\\\\server\\share\\state.json",
        "state\\..\\..\\x.json",
        "state/admin.txt",
        "state/admin",
        "./state/./admin.json/..",
        ".json",
    ] {
        assert!(resolve(root, name).is_err(), "{name:?}");
    }
    let long = format!("{}.json", "a".repeat(MAX_NAME));
    assert!(resolve(root, &long).is_err());
}

#[test]
fn a_file_reached_through_a_link_out_of_out_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let out = root.join(".rbrowser").join("out");
    std::fs::create_dir_all(&out).expect("out");
    let elsewhere = root.join("elsewhere");
    std::fs::create_dir_all(&elsewhere).expect("elsewhere");
    std::fs::write(elsewhere.join("x.json"), "{}").expect("file");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&elsewhere, out.join("state")).expect("link");
        let path = resolve(root, "state/x.json").expect("lexically inside");
        assert!(read(root, &path).is_err(), "the link leaves out/");
    }
    std::fs::write(out.join("ok.json"), "{\"cookies\":[]}").expect("file");
    let path = resolve(root, "ok.json").expect("inside");
    assert_eq!(read(root, &path).expect("read"), "{\"cookies\":[]}");
}

#[test]
fn a_file_too_large_is_not_read() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let out = root.join(".rbrowser").join("out");
    std::fs::create_dir_all(&out).expect("out");
    std::fs::write(out.join("big.json"), vec![b' '; MAX_FILE + 1]).expect("file");
    let path = resolve(root, "big.json").expect("inside");
    assert!(read(root, &path).is_err());
}

#[test]
fn a_written_file_is_the_users_only() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let path = resolve(root, "state/admin.json").expect("inside");
    write(root, &path, b"{}").expect("written");
    assert_eq!(std::fs::read(&path).expect("read"), b"{}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("meta").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    // Written again: replaced, not appended.
    write(root, &path, b"{\"cookies\":[]}").expect("written");
    assert_eq!(std::fs::read(&path).expect("read"), b"{\"cookies\":[]}");
}

/// Security review w28: a committed link `.rbrowser/out` (or `.rbrowser`) would carry the
/// exports wherever the repository points it; such a folder is refused before anything is
/// created or read through it.
#[cfg(unix)]
#[test]
fn an_out_folder_that_is_itself_a_link_is_refused() {
    for linked in [vec![".rbrowser", "out"], vec![".rbrowser"]] {
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path();
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(elsewhere.join("out")).expect("elsewhere");
        let link = linked
            .iter()
            .fold(root.to_path_buf(), |path, part| path.join(part));
        std::fs::create_dir_all(link.parent().expect("parent")).expect("parent");
        let target = if linked.len() == 2 {
            elsewhere.join("out")
        } else {
            elsewhere.clone()
        };
        std::fs::write(target.join("x.json"), "{}").ok();
        std::os::unix::fs::symlink(&target, &link).expect("link");
        let path = resolve(root, "state/admin.json").expect("lexically inside");
        assert!(
            matches!(write(root, &path, b"{}"), Err(PathError::Outside)),
            "{linked:?}"
        );
        assert!(
            !elsewhere.join("out").join("state").exists() && !elsewhere.join("state").exists(),
            "nothing is created through the link"
        );
        let file = resolve(root, "x.json").expect("lexically inside");
        assert!(
            matches!(read(root, &file), Err(PathError::Outside)),
            "{linked:?}"
        );
    }
}
