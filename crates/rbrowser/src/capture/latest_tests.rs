use std::path::Path;

use rb_engine::ViewId;

use super::{recall, remember};
use crate::capture::{ScreenshotAnswer, ShotAnswer};

fn shot(view: u32, path: Option<&str>) -> ShotAnswer {
    ShotAnswer {
        view: Some(ViewId(view)),
        label: format!("view {view}"),
        width: 375,
        height: 812,
        path: path.map(str::to_owned),
        png_base64: path.map(|_| "AAAA".to_owned()),
        error: None,
    }
}

// One test: the record is one per process.
#[test]
fn the_last_saved_screenshots_are_kept_for_their_project_without_image_data() {
    let site = Path::new("/home/me/site");
    assert_eq!(recall(site), None, "nothing before the first");
    let saved = ScreenshotAnswer {
        shots: vec![shot(1, Some("/s/mobile.png")), shot(2, None)],
        overview: None,
        folder: Some("/s".to_owned()),
    };
    remember(site, &saved);
    let kept = recall(site).expect("kept");
    assert_eq!(kept.paths(), ["/s/mobile.png"]);
    assert!(kept.shots.iter().all(|shot| shot.png_base64.is_none()));
    assert_eq!(
        recall(Path::new("/home/me/other")),
        None,
        "another project's"
    );
    // An answer without a file (save false) leaves the last saved ones.
    remember(
        site,
        &ScreenshotAnswer {
            shots: vec![shot(3, None)],
            overview: None,
            folder: None,
        },
    );
    assert_eq!(recall(site).expect("still kept").paths(), ["/s/mobile.png"]);
}
