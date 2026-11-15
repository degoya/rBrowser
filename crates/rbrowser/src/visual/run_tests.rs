use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use rb_engine::EngineError;
use rb_visual::{Rgba, Status, encode_png};

use super::*;
use crate::cdp::lock;

/// A shooter answering each view with the PNG it was given, or an error for any other view;
/// it records the shots asked for.
#[derive(Default)]
struct Pictures {
    by_view: HashMap<ViewId, Vec<u8>>,
    shots: Mutex<Vec<Shot>>,
}

#[async_trait]
impl Shooter for Pictures {
    async fn shoot(&self, view: ViewId, shot: &Shot, _scale: f64) -> Result<Vec<u8>, EngineError> {
        lock(&self.shots).push(shot.clone());
        self.by_view
            .get(&view)
            .cloned()
            .ok_or(EngineError::UnknownView(view))
    }
}

/// A 40 x 30 page (a 2x capture of a 20 x 15 viewport) with a button of `color`.
fn page(color: [u8; 4]) -> Vec<u8> {
    let mut image = Rgba::new(40, 30, [255, 255, 255, 255]).expect("image");
    image.fill(4, 4, 12, 6, color);
    encode_png(&image).expect("png")
}

const BLUE: [u8; 4] = [0, 90, 200, 255];
const RED: [u8; 4] = [200, 30, 30, 255];

fn view(id: u32, name: &str) -> ViewTarget {
    ViewTarget {
        id: ViewId(id),
        label: format!("View {id}"),
        name: name.to_owned(),
        url: "http://user:pw@localhost:5173/shop?token=x".to_owned(),
        engine: "chromium".to_owned(),
        viewport: Viewport {
            width: 20,
            height: 15,
        },
    }
}

fn setup(diff_dir: Option<PathBuf>, engine_version: &str) -> RunSetup {
    RunSetup {
        options: VisualOptions {
            threshold: 0.1,
            full_page: true,
            slug: SlugStrategy::Path,
            lfs_check: true,
            fail_on_meta_mismatch: false,
        },
        machine: BaselineMeta {
            machine: "DEV-01".to_owned(),
            os: "windows".to_owned(),
            engine_version: engine_version.to_owned(),
            app_version: "0.5.0".to_owned(),
            created_at: 1_700_000_000,
            ..BaselineMeta::default()
        },
        diff_dir,
    }
}

fn shooter(pages: &[(u32, Vec<u8>)]) -> Pictures {
    Pictures {
        by_view: pages
            .iter()
            .map(|(id, png)| (ViewId(*id), png.clone()))
            .collect(),
        ..Pictures::default()
    }
}

#[tokio::test]
async fn an_update_stores_each_view_with_its_metadata() {
    let project = tempfile::tempdir().expect("project");
    let store = BaselineStore::new(project.path());
    let pictures = shooter(&[(1, page(BLUE))]);
    let updated = update_views(
        &pictures,
        &store,
        &[view(1, "mobile"), view(2, "desktop")],
        &setup(None, "156"),
    )
    .await;
    assert_eq!(updated.len(), 2);
    assert_eq!(updated[0].slug, "shop");
    assert!(
        updated[0]
            .path
            .as_deref()
            .is_some_and(|path| path.ends_with("mobile.png"))
    );
    assert!(updated[1].error.is_some(), "the unknown view failed alone");
    let stored = store.read("shop", "mobile").expect("read").expect("stored");
    let meta = stored.meta.expect("meta");
    assert_eq!(meta.dpr, 2.0);
    assert_eq!(
        meta.url, "http://localhost:5173/shop",
        "no credentials, no query"
    );
    assert_eq!((meta.machine.as_str(), meta.full_page), ("DEV-01", true));
    assert_eq!(*lock(&pictures.shots), [Shot::FullPage, Shot::FullPage]);
}

#[tokio::test]
async fn an_unchanged_page_is_ok_and_a_changed_button_differs_with_its_images() {
    let project = tempfile::tempdir().expect("project");
    let store = BaselineStore::new(project.path());
    let baseline = shooter(&[(1, page(BLUE)), (2, page(BLUE))]);
    let views = [view(1, "mobile"), view(2, "desktop")];
    update_views(&baseline, &store, &views, &setup(None, "156")).await;

    let diff_dir = project.path().join("shots").join("visual").join("run-1");
    let changed = shooter(&[(1, page(BLUE)), (2, page(RED))]);
    let found = compare_views(
        &changed,
        &store,
        &views,
        &setup(Some(diff_dir.clone()), "156"),
    )
    .await;
    let statuses: Vec<Status> = found.iter().map(|comparison| comparison.status).collect();
    assert_eq!(statuses, [Status::Ok, Status::Differs]);
    assert!(found.iter().all(|comparison| !comparison.unreliable()));
    let differs = &found[1];
    assert_eq!(differs.regions.len(), 1);
    assert_eq!(
        (differs.regions[0].x, differs.regions[0].y),
        (4, 4),
        "the box of the button"
    );
    let expected = diff_dir.join("shop").join("desktop-diff.png");
    assert_eq!(differs.diff_path, Some(expected.display().to_string()));
    assert!(diff_dir.join("shop").join("desktop-actual.png").is_file());
    assert!(
        found[0].diff_path.is_none(),
        "nothing saved for an unchanged view"
    );
}

#[tokio::test]
async fn another_engine_version_is_unreliable_and_a_missing_baseline_is_new() {
    let project = tempfile::tempdir().expect("project");
    let store = BaselineStore::new(project.path());
    let pictures = shooter(&[(1, page(BLUE)), (2, page(BLUE))]);
    update_views(&pictures, &store, &[view(1, "mobile")], &setup(None, "155")).await;
    let found = compare_views(
        &pictures,
        &store,
        &[view(1, "mobile"), view(2, "desktop")],
        &setup(None, "156"),
    )
    .await;
    assert_eq!(found[0].status, Status::Ok);
    assert!(found[0].unreliable());
    assert_eq!(found[0].meta_mismatches[0].field, "engineVersion");
    assert_eq!(found[1].status, Status::New);
}

#[tokio::test]
async fn a_view_whose_file_name_is_taken_or_without_capture_fails_alone() {
    let project = tempfile::tempdir().expect("project");
    let store = BaselineStore::new(project.path());
    let pictures = shooter(&[(1, page(BLUE)), (2, page(BLUE))]);
    let found = compare_views(
        &pictures,
        &store,
        &[
            view(1, "Phone 1"),
            view(2, "Phone/1"),
            view(3, "tablet"),
            view(1, "mobile"),
        ],
        &setup(None, "156"),
    )
    .await;
    let statuses: Vec<Status> = found.iter().map(|comparison| comparison.status).collect();
    assert_eq!(
        statuses,
        [Status::New, Status::Failed, Status::Failed, Status::New]
    );
    assert!(
        found[1]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("Phone-1")),
        "the clash names the file: {:?}",
        found[1].error
    );
    assert!(found[2].error.is_some());
}

#[tokio::test]
async fn a_breakpoint_id_that_is_no_file_name_is_stored_under_its_file_name() {
    let project = tempfile::tempdir().expect("project");
    let store = BaselineStore::new(project.path());
    let pictures = shooter(&[(1, page(BLUE)), (2, page(BLUE))]);
    let views = [view(1, "iPhone 15 Pro"), view(2, "../escape")];
    let updated = update_views(&pictures, &store, &views, &setup(None, "156")).await;
    assert_eq!(updated[0].breakpoint, "iPhone-15-Pro");
    assert_eq!(updated[1].breakpoint, "escape", "nothing leaves the folder");
    assert!(updated.iter().all(|entry| entry.error.is_none()));
    assert!(store.read("shop", "iPhone-15-Pro").expect("read").is_some());
    let found = compare_views(&pictures, &store, &views, &setup(None, "156")).await;
    assert!(
        found
            .iter()
            .all(|comparison| comparison.status == Status::Ok)
    );
}
