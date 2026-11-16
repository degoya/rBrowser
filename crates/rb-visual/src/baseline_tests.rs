use super::*;
use crate::meta::Viewport;

fn meta(engine_version: &str) -> BaselineMeta {
    BaselineMeta {
        engine: "chromium".to_owned(),
        engine_version: engine_version.to_owned(),
        slug: "index".to_owned(),
        viewport: Viewport {
            width: 393,
            height: 852,
        },
        ..BaselineMeta::default()
    }
}

#[test]
fn nothing_stored_reads_as_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = BaselineStore::new(dir.path());
    assert_eq!(store.read("index", "mobile").expect("read"), None);
}

#[test]
fn a_baseline_is_written_below_the_baselines_folder_with_its_meta() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = BaselineStore::new(dir.path());
    let path = store
        .write("index", "mobile", b"png-1", &meta("156"))
        .expect("written");
    let folder = dir.path().join(".rbrowser").join("baselines").join("index");
    assert_eq!(path, folder.join("mobile.png"));
    assert_eq!(path, store.png_path("index", "mobile"));
    let read = store
        .read("index", "mobile")
        .expect("read")
        .expect("stored");
    assert_eq!(read.png, b"png-1");
    assert_eq!(read.meta, Some(meta("156")));
    let file: MetaFile =
        serde_json::from_slice(&std::fs::read(folder.join(META_FILE)).expect("meta.json"))
            .expect("parses");
    assert_eq!(file.baselines.keys().collect::<Vec<_>>(), ["mobile"]);
}

#[test]
fn an_update_replaces_the_baseline_and_keeps_the_other_breakpoints() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = BaselineStore::new(dir.path());
    store
        .write("index", "mobile", b"old", &meta("155"))
        .expect("first");
    store
        .write("index", "desktop", b"desk", &meta("155"))
        .expect("second");
    store
        .write("index", "mobile", b"new", &meta("156"))
        .expect("update");
    let mobile = store
        .read("index", "mobile")
        .expect("read")
        .expect("stored");
    assert_eq!(
        (mobile.png.as_slice(), mobile.meta),
        (&b"new"[..], Some(meta("156")))
    );
    let desktop = store.read("index", "desktop").expect("read").expect("kept");
    assert_eq!(desktop.meta, Some(meta("155")));
    let names: Vec<String> = std::fs::read_dir(dir.path().join(".rbrowser/baselines/index"))
        .expect("folder")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert!(
        names.iter().all(|name| !name.ends_with(".tmp")),
        "no temporary file is left: {names:?}"
    );
}

#[test]
fn a_broken_meta_file_reads_as_no_meta_and_is_replaced() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = BaselineStore::new(dir.path());
    store
        .write("index", "mobile", b"png", &meta("156"))
        .expect("written");
    let meta_path = dir.path().join(".rbrowser/baselines/index").join(META_FILE);
    std::fs::write(&meta_path, b"{ not json").expect("broken");
    let read = store
        .read("index", "mobile")
        .expect("read")
        .expect("stored");
    assert_eq!(read.meta, None);
    store
        .write("index", "mobile", b"png", &meta("157"))
        .expect("rewritten");
    let read = store
        .read("index", "mobile")
        .expect("read")
        .expect("stored");
    assert_eq!(read.meta, Some(meta("157")));
}

#[test]
fn names_that_could_leave_the_folder_are_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = BaselineStore::new(dir.path());
    for (slug, breakpoint) in [
        ("..", "mobile"),
        ("index", "../x"),
        ("a/b", "m"),
        ("index", ""),
    ] {
        assert!(
            store.write(slug, breakpoint, b"x", &meta("1")).is_err(),
            "{slug}/{breakpoint}"
        );
        assert!(store.read(slug, breakpoint).is_err(), "{slug}/{breakpoint}");
    }
}

#[cfg(unix)]
#[test]
fn a_linked_baselines_folder_is_never_followed() {
    let dir = tempfile::tempdir().expect("temp dir");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    std::fs::create_dir(dir.path().join(".rbrowser")).expect(".rbrowser");
    std::os::unix::fs::symlink(elsewhere.path(), dir.path().join(".rbrowser/baselines"))
        .expect("link");
    let store = BaselineStore::new(dir.path());
    assert!(matches!(
        store.write("index", "mobile", b"x", &meta("1")),
        Err(VisualError::Outside(_))
    ));
    assert!(store.read("index", "mobile").is_err());
    assert_eq!(
        std::fs::read_dir(elsewhere.path())
            .expect("elsewhere")
            .count(),
        0,
        "nothing was written through the link"
    );
}

#[cfg(unix)]
#[test]
fn a_linked_baseline_file_is_neither_read_nor_replaced() {
    let dir = tempfile::tempdir().expect("temp dir");
    let secret = dir.path().join("secret.txt");
    std::fs::write(&secret, b"secret").expect("secret");
    let store = BaselineStore::new(dir.path());
    store
        .write("index", "desktop", b"png", &meta("1"))
        .expect("folder made");
    let link = store.png_path("index", "mobile");
    std::os::unix::fs::symlink(&secret, &link).expect("link");
    assert!(store.read("index", "mobile").is_err());
    assert!(store.write("index", "mobile", b"x", &meta("1")).is_err());
    assert_eq!(std::fs::read(&secret).expect("secret"), b"secret");
}

#[test]
fn a_long_page_is_stored_under_its_slug() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = BaselineStore::new(dir.path());
    for length in [70, 79, 80, 200] {
        let page = crate::slug(
            &format!("https://example.com/{}", "a".repeat(length)),
            crate::SlugStrategy::Path,
        );
        let path = store
            .write(&page, "mobile", b"png", &meta("156"))
            .expect("a slug is always a stored name");
        assert!(path.ends_with(format!("{page}/mobile.png")), "{length}");
    }
}
