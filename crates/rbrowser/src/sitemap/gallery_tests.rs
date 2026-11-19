use super::*;

fn open() -> GalleryOpen {
    let file = Path::new("C:\\work\\my shop\\.rbrowser\\out\\sitemap-capture\\r1\\index.html");
    GalleryOpen::new("r1", file, "Gallery r1".to_owned())
}

#[test]
fn the_window_loads_the_galleries_file_address_and_names_its_folder() {
    let open = open();
    assert_eq!(open.run_id, "r1");
    assert_eq!(
        open.url,
        "file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r1/index.html"
    );
    assert_eq!(
        open.folder,
        "file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r1/"
    );
    assert_eq!(open.title, "Gallery r1");
}

#[test]
fn the_window_loads_only_what_lies_in_the_runs_folder() {
    let open = open();
    assert!(open.may_load(&open.url));
    assert!(open.may_load(&format!("{}#page-2", open.url)));
    // Chromium may spell the drive and the escapes its own way.
    assert!(open.may_load("file:///c:/work/my shop/.rbrowser/out/sitemap-capture/r1/index.html"));
    assert!(
        open.may_load("file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r1/home/mobile.png")
    );
    assert!(
        !open.may_load("file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r2/index.html")
    );
    assert!(
        !open.may_load(
            "file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r1/../r2/index.html"
        )
    );
    assert!(!open.may_load(
        "file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r1/%2e%2e/r2/index.html"
    ));
    assert!(
        !open.may_load("file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r10/index.html")
    );
    assert!(!open.may_load("https://www.example.test/"));
    assert!(!open.may_load("rbrowser://ui/"));
}

#[test]
fn a_gallery_put_in_by_a_repository_reaches_nothing_outside_its_folder() {
    // Every request of the window passes `may_load`, images included (security review,
    // wave 090-w21): no SMB share (an NTLM hash), no web beacon, no other file.
    let open = open();
    assert!(!open.may_load("file://attacker/share/x.png"));
    assert!(!open.may_load("file:////attacker/share/x.png"));
    assert!(!open.may_load("http://beacon.test/x.gif"));
    assert!(!open.may_load("data:image/png;base64,AAAA"));
    assert!(!open.may_load("file:///C:/Users/me/.ssh/id_rsa"));
    assert!(!open.may_load(
        "file:///C:/work/my%20shop/.rbrowser/out/sitemap-capture/r1/..%5C..%5Csecret.txt"
    ));
}

#[test]
fn a_runs_gallery_is_its_index_file_once_written() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "r1", true)
        .expect("opens")
        .expect("made");
    assert!(matches!(
        gallery_file(root.path(), "r1"),
        Err(RunError::NoGallery(id)) if id == "r1"
    ));
    store
        .write_file(GALLERY_FILE, b"<!doctype html>")
        .expect("written");
    assert_eq!(
        gallery_file(root.path(), "r1").expect("there"),
        store.dir().join(GALLERY_FILE)
    );
    assert!(matches!(
        gallery_file(root.path(), "r9"),
        Err(RunError::NotFound(id)) if id == "r9"
    ));
    assert!(gallery_file(root.path(), "../r1").is_err());
}

#[test]
fn an_agent_opens_the_gallery_in_the_apps_window_only() {
    // MCP never hands a file of the repository to the system browser (security review, wave
    // 090-w21); the person's command follows the setting.
    assert_eq!(
        opens_in(GalleryOpensIn::System, Caller::Agent),
        GalleryOpensIn::Window
    );
    assert_eq!(
        opens_in(GalleryOpensIn::Window, Caller::Agent),
        GalleryOpensIn::Window
    );
    assert_eq!(
        opens_in(GalleryOpensIn::System, Caller::Person),
        GalleryOpensIn::System
    );
    assert_eq!(
        opens_in(GalleryOpensIn::Window, Caller::Person),
        GalleryOpensIn::Window
    );
}

#[test]
fn the_answer_names_the_run_the_file_and_where_it_opened() {
    let answer = GalleryAnswer {
        run_id: "r1".to_owned(),
        gallery: "C:\\g\\index.html".to_owned(),
        opened_in: GalleryOpensIn::Window,
    };
    assert_eq!(
        serde_json::to_value(answer).expect("json"),
        serde_json::json!({ "runId": "r1", "gallery": "C:\\g\\index.html", "openedIn": "window" })
    );
}
