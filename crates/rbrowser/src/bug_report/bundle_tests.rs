use std::io::Read as _;

use super::*;

#[test]
fn each_report_gets_a_folder_of_its_own() {
    let root = tempfile::tempdir().expect("temp dir");
    let first = new_folder(root.path(), "2026-11-20_14-03-07").expect("first");
    let second = new_folder(root.path(), "2026-11-20_14-03-07").expect("second");
    let reports = root.path().join(".rbrowser/out/reports");
    assert_eq!(first, reports.join("2026-11-20_14-03-07"));
    assert_eq!(second, reports.join("2026-11-20_14-03-07-2"));
    assert!(first.is_dir() && second.is_dir());
}

#[cfg(unix)]
#[test]
fn a_link_on_the_way_is_refused() {
    let root = tempfile::tempdir().expect("temp dir");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    std::fs::create_dir_all(root.path().join(".rbrowser")).expect("rbrowser");
    std::os::unix::fs::symlink(elsewhere.path(), root.path().join(".rbrowser/out")).expect("link");
    assert!(new_folder(root.path(), "x").is_err());
    assert_eq!(
        std::fs::read_dir(elsewhere.path()).expect("read").count(),
        0
    );
}

#[test]
fn a_file_on_the_way_is_refused() {
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(root.path().join(".rbrowser/out")).expect("out");
    std::fs::write(root.path().join(".rbrowser/out/reports"), b"x").expect("file");
    assert!(new_folder(root.path(), "x").is_err());
}

#[test]
fn relative_paths_stay_inside_the_folder() {
    let folder = Path::new("/p/.rbrowser/out/reports/r");
    assert_eq!(
        relative(folder, &folder.join("screens").join("a b.png")).as_deref(),
        Some("screens/a b.png")
    );
    assert_eq!(relative(folder, Path::new("/p/other.png")), None);
    assert_eq!(relative(folder, folder), None);
    assert_eq!(relative(folder, &folder.join("../x.png")), None);
}

#[test]
fn the_zip_holds_every_file_but_itself() {
    let root = tempfile::tempdir().expect("temp dir");
    let folder = new_folder(root.path(), "r").expect("folder");
    std::fs::create_dir_all(folder.join(SCREENS).join("day")).expect("screens");
    std::fs::write(folder.join(SCREENS).join("day").join("mobile.png"), b"png").expect("png");
    write_markdown(&folder, "# report\n").expect("markdown");
    assert!(
        write_markdown(&folder, "again").is_err(),
        "never over a report"
    );
    let zip_path = write_zip(&folder).expect("zip");
    assert_eq!(zip_path, folder.join(ZIP));
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(&zip_path).expect("open")).expect("a zip");
    let mut names: Vec<String> = archive
        .file_names()
        .map(|name| name.expect("a member name").into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["report.md", "screens/day/mobile.png"]);
    let mut text = String::new();
    archive
        .by_name("report.md")
        .expect("entry")
        .read_to_string(&mut text)
        .expect("read");
    assert_eq!(text, "# report\n");
}

#[test]
fn a_chosen_folder_takes_the_report_instead_of_the_project() {
    let root = tempfile::tempdir().expect("temp dir");
    let chosen = tempfile::tempdir().expect("chosen");
    let path = chosen.path().to_str().expect("a UTF-8 temp path");
    let first = new_report_folder(root.path(), Some(path), "r").expect("first");
    let second = new_report_folder(root.path(), Some(path), "r").expect("second");
    assert_eq!(first, chosen.path().join("r"));
    assert_eq!(second, chosen.path().join("r-2"));
    assert!(
        !root.path().join(".rbrowser").exists(),
        "nothing in the project"
    );
    assert_eq!(
        new_report_folder(root.path(), None, "r").expect("the project's"),
        root.path().join(".rbrowser/out/reports/r")
    );
}

#[test]
fn a_chosen_folder_is_an_existing_absolute_folder() {
    let root = tempfile::tempdir().expect("temp dir");
    let chosen = tempfile::tempdir().expect("chosen");
    let missing = chosen.path().join("missing");
    let file = chosen.path().join("file.txt");
    std::fs::write(&file, b"x").expect("file");
    for bad in [
        "tickets".to_owned(),
        missing.display().to_string(),
        file.display().to_string(),
    ] {
        assert!(
            new_report_folder(root.path(), Some(bad.as_str()), "r").is_err(),
            "{bad}"
        );
    }
    assert!(!missing.exists(), "a missing folder is not made");
}

#[test]
fn a_discarded_report_leaves_nothing_behind() {
    let root = tempfile::tempdir().expect("temp dir");
    let folder = new_folder(root.path(), "r").expect("folder");
    std::fs::create_dir_all(folder.join(SCREENS)).expect("screens");
    std::fs::write(folder.join(SCREENS).join("mobile.png"), b"png").expect("png");
    discard(&folder);
    assert!(!folder.exists());
    assert!(
        root.path().join(".rbrowser/out/reports").is_dir(),
        "only the report's own folder goes"
    );
    discard(&folder);
}
