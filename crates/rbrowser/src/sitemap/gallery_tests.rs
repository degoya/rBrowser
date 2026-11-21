use super::*;

#[test]
fn the_window_shows_the_gallery_page_of_the_shells_pages() {
    let open = GalleryOpen::new("2026-11-11_10-00-00", "Gallery".to_owned()).expect("a plain id");
    assert_eq!(open.run_id, "2026-11-11_10-00-00");
    let page = "rbrowser://ui/settings.html#/sitemap-gallery?run=2026-11-11_10-00-00&open=";
    assert!(open.url.starts_with(page), "{}", open.url);
    assert!(crate::shell::is_shell_url(&open.url));
    assert_eq!(open.title, "Gallery");
    // The same run opened again gets another address: the page reads it again.
    let again = GalleryOpen::new("2026-11-11_10-00-00", "Gallery".to_owned()).expect("a plain id");
    assert_ne!(again.url, open.url);
}

#[test]
fn a_run_id_that_is_no_plain_name_opens_nothing() {
    for id in ["../r1", "r1#x", "r1&run=r2", "", "r 1"] {
        assert_eq!(GalleryOpen::new(id, String::new()), None, "{id}");
    }
}

#[test]
fn a_running_run_opens_in_the_window_before_its_index_file_is_written() {
    // RB-090-88: the window reads `run.json`, a running run's too; only the system browser needs
    // the `index.html` the plugin writes when the run ends.
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "r1", true)
        .expect("opens")
        .expect("made");
    assert_eq!(
        gallery_of(root.path(), "r1", GalleryOpensIn::Window).expect("opens"),
        None
    );
    assert!(matches!(
        gallery_of(root.path(), "r1", GalleryOpensIn::System),
        Err(RunError::NoGallery(id)) if id == "r1"
    ));
    store
        .write_file(GALLERY_FILE, b"<!doctype html>")
        .expect("written");
    let file = store.dir().join(GALLERY_FILE);
    for opened_in in [GalleryOpensIn::Window, GalleryOpensIn::System] {
        assert_eq!(
            gallery_of(root.path(), "r1", opened_in).expect("there"),
            Some(file.clone())
        );
        assert!(matches!(
            gallery_of(root.path(), "r9", opened_in),
            Err(RunError::NotFound(id)) if id == "r9"
        ));
        assert!(gallery_of(root.path(), "../r1", opened_in).is_err());
    }
}

#[test]
fn a_folder_named_like_the_index_file_is_no_gallery() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "r1", true)
        .expect("opens")
        .expect("made");
    std::fs::create_dir(store.dir().join(GALLERY_FILE)).expect("a folder");
    assert_eq!(
        gallery_of(root.path(), "r1", GalleryOpensIn::Window).expect("opens"),
        None
    );
    assert!(matches!(
        gallery_of(root.path(), "r1", GalleryOpensIn::System),
        Err(RunError::NoGallery(_))
    ));
}

#[test]
fn the_system_browser_without_an_index_file_tells_the_person_it_comes_at_the_end() {
    let CoreToShell::Notification(notification) = at_end_notice("r1") else {
        panic!("a notification");
    };
    assert_eq!(notification.level, NotificationLevel::Info);
    assert_eq!(notification.message_key, "sitemap.notice.galleryAtEnd");
    assert_eq!(notification.args["runId"], "r1");
    // The link opens the setting that chooses the window, which shows a running run.
    assert_eq!(
        notification.settings_key.as_deref(),
        Some("sitemap.galleryOpensIn")
    );
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
        gallery: Some("C:\\g\\index.html".to_owned()),
        opened_in: GalleryOpensIn::Window,
    };
    assert_eq!(
        serde_json::to_value(answer).expect("json"),
        serde_json::json!({ "runId": "r1", "gallery": "C:\\g\\index.html", "openedIn": "window" })
    );
}

#[test]
fn a_running_runs_answer_has_no_gallery_file_yet() {
    let answer = GalleryAnswer {
        run_id: "r1".to_owned(),
        gallery: None,
        opened_in: GalleryOpensIn::Window,
    };
    assert_eq!(
        serde_json::to_value(answer).expect("json"),
        serde_json::json!({ "runId": "r1", "gallery": null, "openedIn": "window" })
    );
}
