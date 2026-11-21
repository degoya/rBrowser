use std::collections::BTreeMap;

use rb_visual::sitemap::{RunBreakpoint, RunPage, RunState, Shot, ShotCompare, ShotState};
use serde_json::json;

use super::*;

const RUN: &str = "2026-11-11_10-00-00";

fn shot(state: ShotState) -> Shot {
    Shot {
        state,
        ..Shot::default()
    }
}

fn run() -> SitemapRun {
    let breakpoints = vec![
        RunBreakpoint {
            name: "mobile".to_owned(),
            label: "Mobile".to_owned(),
            width: 375,
            height: 812,
            dpr: 2.0,
        },
        RunBreakpoint {
            name: "desktop".to_owned(),
            label: String::new(),
            width: 1440,
            height: 900,
            dpr: 1.0,
        },
    ];
    let mut done = shot(ShotState::Done);
    done.compare = Some(ShotCompare {
        status: "differs".to_owned(),
        ratio: 0.0123,
        ..ShotCompare::default()
    });
    let mut failed = shot(ShotState::Failed);
    failed.error = Some("timeout".to_owned());
    let index = RunPage {
        url: "http://localhost:3000/".to_owned(),
        slug: "index".to_owned(),
        status: Some(200),
        shots: BTreeMap::from([("mobile".to_owned(), done), ("desktop".to_owned(), failed)]),
        ..RunPage::default()
    };
    let hostile = RunPage {
        url: "http://localhost:3000/old".to_owned(),
        slug: "../../etc".to_owned(),
        status: Some(404),
        final_url: Some("http://localhost:3000/new".to_owned()),
        shots: BTreeMap::from([("mobile".to_owned(), shot(ShotState::Done))]),
        ..RunPage::default()
    };
    SitemapRun {
        run_id: RUN.to_owned(),
        source: "http://localhost:3000/".to_owned(),
        state: RunState::Finished,
        breakpoints,
        pages: vec![index, hostile],
        ..SitemapRun::default()
    }
}

#[test]
fn a_shot_address_names_the_run_the_page_and_the_breakpoint() {
    let url = shot_url(RUN, "index", "mobile").expect("plain names");
    assert_eq!(url, format!("rbrowser://sitemap/{RUN}/index/mobile.png"));
    assert_eq!(
        parse_shot_url(&url),
        Some(ShotAddress {
            run_id: RUN.to_owned(),
            slug: "index".to_owned(),
            breakpoint: "mobile".to_owned(),
        })
    );
    // Chromium may write the scheme and host in its own case.
    assert!(parse_shot_url(&format!("RBROWSER://Sitemap/{RUN}/index/mobile.png")).is_some());
    assert_eq!(shot_url(RUN, "../x", "mobile"), None);
    assert_eq!(shot_url("..", "index", "mobile"), None);
}

#[test]
fn an_address_outside_one_runs_shots_is_no_shot() {
    for url in [
        "rbrowser://sitemap/r1/index/mobile.jpg".to_owned(),
        "rbrowser://sitemap/r1/index/mobile".to_owned(),
        "rbrowser://sitemap/r1/mobile.png".to_owned(),
        "rbrowser://sitemap/r1/a/b/mobile.png".to_owned(),
        "rbrowser://sitemap/../index/mobile.png".to_owned(),
        "rbrowser://sitemap/r1/../mobile.png".to_owned(),
        "rbrowser://sitemap/r1/%2e%2e/mobile.png".to_owned(),
        "rbrowser://sitemap/r1/index/..png".to_owned(),
        "rbrowser://sitemap/r1/index/mobile.png?x=1".to_owned(),
        "rbrowser://sitemap/r1/index/mobile.png#x".to_owned(),
        "rbrowser://sitemap/r1/index%5Cmobile.png".to_owned(),
        "rbrowser://video/r1/index/mobile.png".to_owned(),
        "rbrowser://sitemap//index/mobile.png".to_owned(),
        "file:///C:/r1/index/mobile.png".to_owned(),
        format!("rbrowser://sitemap/{}/index/mobile.png", "a".repeat(300)),
    ] {
        assert_eq!(parse_shot_url(&url), None, "{url}");
    }
}

/// The bytes of the shot [`open_shot`] opens.
fn served(root: Option<&Path>, shown: Option<&str>, url: &str) -> Result<Vec<u8>, ShotRefusal> {
    use std::io::Read as _;
    open_shot(root, shown, url).map(|mut shot| {
        let mut bytes = Vec::new();
        shot.file.read_to_end(&mut bytes).expect("reads");
        bytes
    })
}

#[test]
fn a_shot_longer_than_the_galleries_limit_is_refused() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), RUN, true)
        .expect("opens")
        .expect("made");
    let long = vec![0_u8; usize::try_from(MAX_GALLERY_SHOT_BYTES + 1).expect("fits")];
    store.write_shot("index", "mobile", &long).expect("writes");
    let url = format!("rbrowser://sitemap/{RUN}/index/mobile.png");
    assert_eq!(
        served(Some(root.path()), Some(RUN), &url),
        Err(ShotRefusal::Unreadable)
    );
}

#[test]
fn a_shot_is_read_from_the_shown_run_only() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), RUN, true)
        .expect("opens")
        .expect("made");
    store
        .write_shot("index", "mobile", b"\x89PNG")
        .expect("writes");
    let other = RunStore::open(root.path(), "2026-11-12_10-00-00", true)
        .expect("opens")
        .expect("made");
    other
        .write_shot("index", "mobile", b"\x89PNG")
        .expect("writes");
    let url = format!("rbrowser://sitemap/{RUN}/index/mobile.png");
    assert_eq!(
        served(Some(root.path()), Some(RUN), &url).expect("served"),
        b"\x89PNG"
    );
    let elsewhere = "rbrowser://sitemap/2026-11-12_10-00-00/index/mobile.png";
    assert_eq!(
        served(Some(root.path()), Some(RUN), elsewhere),
        Err(ShotRefusal::OtherRun)
    );
    assert_eq!(
        served(Some(root.path()), None, &url),
        Err(ShotRefusal::OtherRun)
    );
    assert_eq!(served(None, Some(RUN), &url), Err(ShotRefusal::NoProject));
    assert_eq!(
        served(
            Some(root.path()),
            Some(RUN),
            &format!("rbrowser://sitemap/{RUN}/index/desktop.png")
        ),
        Err(ShotRefusal::Missing)
    );
    assert_eq!(
        served(
            Some(root.path()),
            Some(RUN),
            &format!("rbrowser://sitemap/{RUN}/../mobile.png")
        ),
        Err(ShotRefusal::Address)
    );
    // run.json and the gallery are no shots.
    assert_eq!(
        served(
            Some(root.path()),
            Some(RUN),
            &format!("rbrowser://sitemap/{RUN}/run.json")
        ),
        Err(ShotRefusal::Address)
    );
}

#[cfg(unix)]
#[test]
fn a_shot_behind_a_link_is_refused() {
    let root = tempfile::tempdir().expect("temp");
    let elsewhere = tempfile::tempdir().expect("temp");
    std::fs::write(elsewhere.path().join("mobile.png"), b"secret").expect("writes");
    let store = RunStore::open(root.path(), RUN, true)
        .expect("opens")
        .expect("made");
    std::os::unix::fs::symlink(elsewhere.path(), store.dir().join("index")).expect("link");
    let url = format!("rbrowser://sitemap/{RUN}/index/mobile.png");
    assert_eq!(
        served(Some(root.path()), Some(RUN), &url),
        Err(ShotRefusal::Unreadable)
    );
}

#[test]
fn the_page_has_every_breakpoint_and_page_and_images_only_for_plain_done_shots() {
    let page = page_of(&run(), 480);
    assert_eq!(page.run_id, RUN);
    assert_eq!(page.state, "finished");
    assert_eq!(page.thumbnail_width, 480);
    assert_eq!(page.breakpoints.len(), 2);
    assert_eq!(page.breakpoints[1].label, "");
    let index = &page.pages[0];
    assert_eq!(
        index.shots["mobile"].image.as_deref(),
        Some(format!("rbrowser://sitemap/{RUN}/index/mobile.png").as_str())
    );
    assert_eq!(
        index.shots["mobile"].compare.as_ref().map(|c| c.ratio),
        Some(0.0123)
    );
    assert_eq!(index.shots["desktop"].state, "failed");
    assert_eq!(index.shots["desktop"].error.as_deref(), Some("timeout"));
    assert_eq!(index.shots["desktop"].image, None);
    let hostile = &page.pages[1];
    assert_eq!(hostile.status, Some(404));
    assert_eq!(
        hostile.final_url.as_deref(),
        Some("http://localhost:3000/new")
    );
    assert_eq!(hostile.shots["mobile"].state, "done");
    assert_eq!(
        hostile.shots["mobile"].image, None,
        "a slug that is no plain name"
    );
}

fn schema(choices: serde_json::Value, default: u32) -> serde_json::Value {
    let key = json!({ "type": "integer", "enum": choices, "default": default });
    json!({ "properties": { "thumbnailWidth": key } })
}

#[test]
fn the_thumbnail_width_is_a_choice_of_the_plugins_loaded_schema() {
    let loaded = schema(json!([200, 400]), 400);
    assert_eq!(thumbnail_width(Some(&json!(200)), Some(&loaded)), 200);
    for odd in [
        json!(160),
        json!("200"),
        json!(-1),
        json!(null),
        json!(100_000),
    ] {
        assert_eq!(thumbnail_width(Some(&odd), Some(&loaded)), 400, "{odd}");
    }
    assert_eq!(thumbnail_width(None, Some(&loaded)), 400);
}

#[test]
fn without_a_loaded_plugin_the_bundled_schema_decides() {
    // The plugin's own settings.schema.json is the one source of the choices and the default.
    let bundled: serde_json::Value =
        serde_json::from_str(BUNDLED_SCHEMA).expect("the bundled schema");
    let key = &bundled["properties"]["thumbnailWidth"];
    let default = u32::try_from(key["default"].as_u64().expect("a default")).expect("fits");
    assert!(
        key["enum"]
            .as_array()
            .is_some_and(|choices| choices.contains(&json!(default)))
    );
    assert_eq!(thumbnail_width(None, None), default);
    assert_eq!(thumbnail_width(Some(&json!(640)), None), 640);
    assert_eq!(thumbnail_width(Some(&json!(333)), None), default);
    // A loaded schema without the key falls back to the bundled one.
    assert_eq!(thumbnail_width(Some(&json!(640)), Some(&json!({}))), 640);
    assert_eq!(
        THUMBNAIL_SETTING,
        format!("{}.thumbnailWidth", crate::sitemap::app::PLUGIN_ID)
    );
}

#[test]
fn the_answer_carries_at_most_max_pages_and_says_how_many_there_are() {
    let mut long = run();
    let template = long.pages[0].clone();
    long.pages = (0..MAX_PAGES + 5)
        .map(|at| {
            let mut page = template.clone();
            page.url = format!("http://localhost:3000/p{at}");
            page
        })
        .collect();
    let page = page_of(&long, 320);
    assert_eq!(page.pages.len(), MAX_PAGES);
    assert_eq!(
        page.total_pages,
        Some(u32::try_from(MAX_PAGES + 5).expect("fits"))
    );
    assert_eq!(page_of(&run(), 320).total_pages, None);
}

#[test]
fn the_gallery_request_is_answered_off_the_ui_thread() {
    assert!(is_gallery_request(&ShellToCore::GetSitemapGallery(
        rb_ipc::SitemapGalleryRequest::default()
    )));
    assert!(!is_gallery_request(&ShellToCore::GetState));
}

#[test]
fn a_run_is_loaded_from_its_folder_and_a_missing_one_is_not_found() {
    let root = tempfile::tempdir().expect("temp");
    let store = RunStore::open(root.path(), RUN, true)
        .expect("opens")
        .expect("made");
    store.save(&run()).expect("saves");
    let page = load(root.path(), RUN, 320).expect("loads");
    assert_eq!(page.pages.len(), 2);
    assert!(matches!(
        load(root.path(), "2026-01-01_00-00-00", 320),
        Err(RunError::NotFound(_))
    ));
    assert!(load(root.path(), "../x", 320).is_err());
}

#[test]
fn only_the_gallery_windows_main_frame_gets_a_shot() {
    assert!(requester_allowed(23, 23, true));
    assert!(!requester_allowed(23, 23, false), "a frame in the page");
    assert!(!requester_allowed(23, 7, true), "another browser");
    assert!(!requester_allowed(0, 0, true), "the window is closed");
}
