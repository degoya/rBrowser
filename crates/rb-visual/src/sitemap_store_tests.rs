use super::*;
use crate::sitemap::{Outcome, RunBreakpoint};

fn run() -> SitemapRun {
    let breakpoints = vec![RunBreakpoint {
        name: "mobile".to_owned(),
        label: "Mobile".to_owned(),
        width: 375,
        height: 812,
        dpr: 2.0,
    }];
    let pages = vec![("https://a.test/about".to_owned(), None)];
    SitemapRun::new(
        "2026-11-11_00-00-00",
        "https://a.test/",
        breakpoints,
        &pages,
        0,
    )
}

#[test]
fn a_run_is_saved_loaded_and_its_shots_written_below_its_folder() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "2026-11-11_00-00-00", true)
        .expect("opens")
        .expect("created");
    assert_eq!(
        store.dir(),
        root.path()
            .join(".rbrowser/out/sitemap-capture/2026-11-11_00-00-00")
    );
    let mut run = run();
    let file = store.write_shot("about", "mobile", b"png").expect("writes");
    assert_eq!(file, "about/mobile.png");
    assert_eq!(
        std::fs::read(store.dir().join(&file)).expect("reads"),
        b"png"
    );
    let again = store
        .write_shot("about", "mobile", b"new")
        .expect("replaces");
    assert_eq!(again, file);
    assert_eq!(
        std::fs::read(store.dir().join(&file)).expect("reads"),
        b"new"
    );
    let task = run.tasks(false, 3)[0].clone();
    run.record(
        &task,
        Outcome {
            status: Some(200),
            final_url: None,
            result: Ok(file),
            ms: 1,
        },
    );
    store.save(&run).expect("saves");
    assert_eq!(store.load().expect("loads"), Some(run));
    store.write_file(GALLERY_FILE, b"<html>").expect("gallery");
    assert!(store.dir().join(GALLERY_FILE).is_file());
    assert_eq!(
        run_ids(root.path()).expect("lists"),
        ["2026-11-11_00-00-00"]
    );
}

#[test]
fn a_missing_run_is_none_and_names_that_leave_the_folder_are_refused() {
    let root = tempfile::tempdir().expect("temp");
    assert!(
        RunStore::open(root.path(), "nope", false)
            .expect("ok")
            .is_none()
    );
    assert!(run_ids(root.path()).expect("empty").is_empty());
    assert!(RunStore::open(root.path(), "../x", true).is_err());
    assert!(RunStore::open(root.path(), "..", true).is_err());
    let store = RunStore::open(root.path(), "r", true)
        .expect("ok")
        .expect("made");
    assert!(store.write_shot("../up", "mobile", b"x").is_err());
    assert!(store.write_shot("about", "../mobile", b"x").is_err());
    assert!(store.write_file("../index.html", b"x").is_err());
    assert!(store.load().expect("no file").is_none());
}

#[cfg(unix)]
#[test]
fn a_link_on_the_way_is_refused() {
    let root = tempfile::tempdir().expect("temp");
    let elsewhere = tempfile::tempdir().expect("temp");
    std::fs::create_dir_all(root.path().join(".rbrowser/out")).expect("dirs");
    std::os::unix::fs::symlink(
        elsewhere.path(),
        root.path().join(".rbrowser/out/sitemap-capture"),
    )
    .expect("link");
    assert!(RunStore::open(root.path(), "r", true).is_err());
    assert!(run_ids(root.path()).is_err());
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "r", true)
        .expect("ok")
        .expect("made");
    std::os::unix::fs::symlink(elsewhere.path(), store.dir().join("about")).expect("link");
    assert!(store.write_shot("about", "mobile", b"x").is_err());
    assert!(
        std::fs::read_dir(elsewhere.path())
            .expect("lists")
            .next()
            .is_none()
    );
}

#[test]
fn a_broken_run_json_is_an_error() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "r", true)
        .expect("ok")
        .expect("made");
    std::fs::write(store.dir().join(RUN_FILE), b"{not json").expect("writes");
    assert!(store.load().is_err());
}

#[test]
fn the_run_id_is_the_stamps_date_and_time() {
    assert_eq!(run_id(&Stamp::from_unix(0)), "1970-01-01_00-00-00");
}

#[test]
fn a_long_page_slug_is_written() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "2026-11-11_00-00-00", true)
        .expect("opens")
        .expect("created");
    let pages = vec![(format!("https://a.test/{}", "p".repeat(70)), None)];
    let run = SitemapRun::new("r", "https://a.test/", Vec::new(), &pages, 0);
    let page_slug = &run.pages[0].slug;
    store
        .write_shot(page_slug, "mobile", b"png")
        .expect("the run's own slug is written");
}

#[test]
fn a_store_opens_below_another_output_folder_and_lists_its_runs() {
    let root = tempfile::tempdir().expect("temp");
    let out = [".rbrowser", "out", "env-compare"];
    let store = RunStore::open_in(root.path(), &out, "2026-11-12_00-00-00", true)
        .expect("opens")
        .expect("created");
    assert_eq!(
        store.dir(),
        root.path()
            .join(".rbrowser/out/env-compare/2026-11-12_00-00-00")
    );
    assert_eq!(
        run_ids_in(root.path(), &out).expect("lists"),
        ["2026-11-12_00-00-00"]
    );
    assert!(run_ids(root.path()).expect("lists").is_empty());
    assert!(RunStore::open_in(root.path(), &out, "../x", false).is_err());
    store.write_file("report.json", b"{}").expect("writes");
    assert_eq!(
        store.read("report.json", 16).expect("reads"),
        Some(b"{}".to_vec())
    );
    assert_eq!(store.read("missing.json", 16).expect("reads"), None);
    assert!(store.read("report.json", 1).is_err(), "too large");
    assert!(store.read("../report.json", 16).is_err());
}

#[test]
fn runs_are_ordered_by_when_their_folder_was_made_not_by_name() {
    // RB-090-69: names in local time do not sort by time across the fall back of daylight
    // saving time, nor after folders named in UTC.
    let at = |secs| Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs));
    let runs = vec![
        (at(1_793_000_000 + 1800), "2026-10-25_02-15-00".to_owned()),
        (at(1_793_000_000), "2026-10-25_02-45-00".to_owned()),
        (at(1_792_000_000), "2026-10-13_09-00-00".to_owned()),
        (at(1_792_000_000), "2026-10-13_08-00-00".to_owned()),
        (None, "undated".to_owned()),
    ];
    assert_eq!(
        oldest_first(runs),
        [
            "undated",
            "2026-10-13_08-00-00",
            "2026-10-13_09-00-00",
            "2026-10-25_02-45-00",
            "2026-10-25_02-15-00",
        ]
    );
}

/// The bytes of the shot `open_shot` opens, `None` when it is not there.
fn shot(
    store: &RunStore,
    slug: &str,
    breakpoint: &str,
    limit: u64,
) -> Result<Option<Vec<u8>>, VisualError> {
    use std::io::Read as _;
    store.open_shot(slug, breakpoint, limit).map(|opened| {
        opened.map(|mut opened| {
            let mut bytes = Vec::new();
            opened.file.read_to_end(&mut bytes).expect("reads");
            assert_eq!(opened.length, bytes.len() as u64);
            bytes
        })
    })
}

#[test]
fn a_shot_is_opened_by_its_plain_names_only() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), "r", true)
        .expect("ok")
        .expect("made");
    store.write_shot("about", "mobile", b"png").expect("writes");
    assert_eq!(
        shot(&store, "about", "mobile", 1024).expect("reads"),
        Some(b"png".to_vec())
    );
    assert_eq!(shot(&store, "about", "desktop", 1024).expect("reads"), None);
    assert_eq!(
        shot(&store, "missing", "mobile", 1024).expect("reads"),
        None
    );
    assert!(
        shot(&store, "about", "mobile", 2).is_err(),
        "longer than the limit"
    );
    for (slug, breakpoint) in [
        ("..", "mobile"),
        ("about", ".."),
        ("about/..", "mobile"),
        ("about", "mobile/../../x"),
        ("", "mobile"),
        ("about", "mobile.png\\x"),
    ] {
        assert!(
            shot(&store, slug, breakpoint, 1024).is_err(),
            "{slug} / {breakpoint}"
        );
    }
    std::fs::create_dir(store.dir().join("about/folder.png")).expect("dir");
    assert!(
        shot(&store, "about", "folder", 1024).is_err(),
        "a folder is no shot"
    );
}

#[cfg(unix)]
#[test]
fn a_shot_behind_a_link_is_refused() {
    let root = tempfile::tempdir().expect("temp");
    let elsewhere = tempfile::tempdir().expect("temp");
    std::fs::write(elsewhere.path().join("mobile.png"), b"secret").expect("writes");
    let store = RunStore::open(root.path(), "r", true)
        .expect("ok")
        .expect("made");
    std::os::unix::fs::symlink(elsewhere.path(), store.dir().join("about")).expect("link");
    assert!(shot(&store, "about", "mobile", 1024).is_err());
    std::fs::create_dir(store.dir().join("page")).expect("dir");
    std::os::unix::fs::symlink(
        elsewhere.path().join("mobile.png"),
        store.dir().join("page/mobile.png"),
    )
    .expect("link");
    assert!(shot(&store, "page", "mobile", 1024).is_err());
}
