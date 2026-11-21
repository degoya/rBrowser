use std::fs;

use rb_engine::ViewId;

use super::{PNG_SIGNATURE, screenshot_png, screenshots};
use crate::capture::{ScreenshotAnswer, ShotAnswer, latest};

fn shot(view: u32, path: &str) -> ShotAnswer {
    ShotAnswer {
        view: Some(ViewId(view)),
        label: format!("view {view}"),
        width: 2,
        height: 2,
        path: Some(path.to_owned()),
        png_base64: None,
        error: None,
    }
}

// One test: the record is one per process.
#[test]
fn the_files_of_the_last_screenshots_are_read_by_index_within_the_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    assert!(screenshots(Some(root)).is_none());
    assert_eq!(
        screenshot_png(Some(root), 0, 1024)
            .expect_err("none yet")
            .code,
        "screenshots_none"
    );
    let first = root.join("mobile.png");
    let overview = root.join("overview.png");
    let png = [PNG_SIGNATURE.as_slice(), b"IHDR".as_slice()].concat();
    fs::write(&first, &png).expect("written");
    fs::write(&overview, vec![7; 2000]).expect("written");
    latest::remember(
        root,
        &ScreenshotAnswer {
            shots: vec![shot(1, &first.to_string_lossy())],
            overview: Some(ShotAnswer {
                view: None,
                ..shot(0, &overview.to_string_lossy())
            }),
            folder: Some(root.to_string_lossy().into_owned()),
        },
    );
    let listed = screenshots(Some(root)).expect("listed");
    assert_eq!(listed["shots"][0]["path"], first.to_string_lossy().as_ref());
    assert_eq!(screenshot_png(Some(root), 0, 1024).expect("the first"), png);
    // The overview is the last file; over the limit it is refused, not cut.
    let refused = screenshot_png(Some(root), 1, 1024).expect_err("too large");
    assert_eq!(refused.code, "answer_too_large");
    assert_eq!(refused.settings_key.as_deref(), Some("mcp.maxAnswerBytes"));
    assert_eq!(
        screenshot_png(Some(root), 2, 1024)
            .expect_err("past the end")
            .code,
        "screenshots_none"
    );
    // Only a PNG is answered: a file put in its place that is none is refused.
    fs::write(&first, b"not an image").expect("written");
    assert_eq!(
        screenshot_png(Some(root), 0, 1024)
            .expect_err("no PNG")
            .code,
        "screenshots_none"
    );
    // A link put in its place is not followed, even to a PNG.
    #[cfg(unix)]
    {
        let elsewhere = root.join("elsewhere.png");
        fs::write(&elsewhere, &png).expect("written");
        fs::remove_file(&first).expect("removed");
        std::os::unix::fs::symlink(&elsewhere, &first).expect("a link");
        assert_eq!(
            screenshot_png(Some(root), 0, 1024)
                .expect_err("a link")
                .code,
            "screenshots_none"
        );
    }
    fs::remove_file(first).expect("removed");
    assert_eq!(
        screenshot_png(Some(root), 0, 1024).expect_err("gone").code,
        "screenshots_none"
    );
    assert!(screenshots(None).is_none(), "no project, nothing");
}
