use std::fs;

use super::*;

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    fs::create_dir_all(dir.path().join("fixtures/api/users")).expect("dirs");
    fs::write(dir.path().join("fixtures/api/config.json"), b"{\"a\":1}").expect("file");
    fs::write(
        dir.path().join("fixtures/api/users/index.html"),
        b"<p>u</p>",
    )
    .expect("file");
    fs::write(dir.path().join("fixtures/api/users/42"), b"{\"id\":42}").expect("file");
    dir
}

#[test]
fn a_folder_resolves_below_the_project_or_an_allowed_root_and_nowhere_else() {
    let project = project();
    let outside = tempfile::tempdir().expect("a temp dir");
    fs::create_dir_all(outside.path().join("shared")).expect("dir");
    let root = project.path();
    let resolved = resolve("fixtures/api", Some(root), &[]).expect("below the project");
    assert_eq!(
        resolved,
        root.join("fixtures/api").canonicalize().expect("canonical")
    );
    let absolute = root.join("fixtures").display().to_string();
    assert!(
        resolve(&absolute, Some(root), &[]).is_ok(),
        "absolute, below the project"
    );
    let elsewhere = outside.path().join("shared").display().to_string();
    assert_eq!(
        resolve(&elsewhere, Some(root), &[]),
        Err("folder outside the project and the allowed roots")
    );
    let allowed = outside.path().display().to_string();
    assert!(
        resolve(&elsewhere, Some(root), &[allowed]).is_ok(),
        "an allowed root"
    );
    assert_eq!(
        resolve("fixtures/../../x", Some(root), &[]),
        Err("folder does not exist")
    );
    assert_eq!(resolve("fixtures/api", None, &[]), Err("no project open"));
    let escape = format!("{}/../", root.display());
    assert_eq!(
        resolve(&escape, Some(root), &[]),
        Err("folder outside the project and the allowed roots")
    );
}

#[test]
fn the_urls_path_finds_a_file_below_the_folder_but_never_above_it() {
    let project = project();
    let folder = resolve("fixtures/api", Some(project.path()), &[]).expect("folder");
    let file = |url: &str| file_for(&folder, url);
    assert_eq!(
        file("https://a.test/config.json?v=2").expect("found"),
        folder.join("config.json")
    );
    assert_eq!(
        file("https://a.test/users/").expect("found"),
        folder.join("users").join("index.html"),
        "a folder serves its index"
    );
    assert_eq!(
        file("https://a.test/users/42").expect("found"),
        folder.join("users").join("42")
    );
    assert_eq!(
        file("https://a.test/users/%34%32").expect("found"),
        folder.join("users").join("42"),
        "percent-decoded"
    );
    assert_eq!(
        file("https://a.test/missing.json"),
        Err(LocalError::NotFound)
    );
    assert_eq!(
        file("https://a.test/../config.json"),
        Err(LocalError::Outside)
    );
    assert_eq!(
        file("https://a.test/users/..%2F..%2Fconfig.json"),
        Err(LocalError::Outside)
    );
    assert_eq!(file("https://a.test/users/x%00"), Err(LocalError::Outside));
    assert_eq!(file("https://a.test/c:\\x"), Err(LocalError::Outside));
}

#[test]
fn the_mime_follows_the_extension() {
    assert_eq!(mime_of(Path::new("a/config.json")), "application/json");
    assert_eq!(mime_of(Path::new("x.HTML")), "text/html");
    assert_eq!(mime_of(Path::new("x.css")), "text/css");
    assert_eq!(mime_of(Path::new("x.mjs")), "text/javascript");
    assert_eq!(mime_of(Path::new("x.webp")), "image/webp");
    assert_eq!(mime_of(Path::new("x")), "application/octet-stream");
}
