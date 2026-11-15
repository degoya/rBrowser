use super::*;
use crate::testing::host;

const PNG: &[u8] = b"png";

#[test]
fn the_last_screenshot_is_in_the_newest_run() {
    let mut host = host()
        .file(
            ".rbrowser/out/screenshots/2026-11-10_09-00-00/mobile.png",
            PNG,
        )
        .file(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/desktop.png",
            PNG,
        )
        .file(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/meta.json",
            b"{}",
        )
        .file(
            ".rbrowser/out/screenshots/2026-11-11_23-59-59/tablet.png",
            PNG,
        );
    assert_eq!(
        last(&mut host),
        Ok(Some(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/desktop.png".to_owned()
        ))
    );
}

#[test]
fn a_pattern_with_more_folders_is_walked_down() {
    let mut host = host()
        .file(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/staging/mobile.png",
            PNG,
        )
        .file(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/production/mobile.png",
            PNG,
        );
    assert_eq!(
        last(&mut host),
        Ok(Some(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/staging/mobile.png".to_owned()
        ))
    );
}

#[test]
fn without_screenshots_there_is_no_last_one() {
    assert_eq!(last(&mut host()), Ok(None));
    let mut other = host().file(".rbrowser/out/screenshots/notes.txt", b"x");
    assert_eq!(last(&mut other), Ok(None));
}

#[test]
fn the_list_has_the_screenshots_first_then_the_other_pngs_but_never_the_own_folder() {
    let mut host = host()
        .file(
            ".rbrowser/out/screenshots/2026-11-10_09-00-00/mobile.png",
            PNG,
        )
        .file(
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/desktop.PNG",
            PNG,
        )
        .file(".rbrowser/out/checks/run-1/diff.png", PNG)
        .file(
            ".rbrowser/out/screenshot-editor/screenshots/x/mobile.edited.png",
            PNG,
        )
        .file(".rbrowser/out/fonts/report.json", b"{}");
    let (images, truncated) = list(&mut host).expect("list");
    assert_eq!(
        images,
        vec![
            ".rbrowser/out/screenshots/2026-11-12_10-00-00/desktop.PNG",
            ".rbrowser/out/screenshots/2026-11-10_09-00-00/mobile.png",
            ".rbrowser/out/checks/run-1/diff.png",
        ]
    );
    assert!(!truncated);
}

#[test]
fn the_list_stops_at_its_limit() {
    let mut host = host();
    for index in 0..MAX_LISTED + 5 {
        host = host.file(
            &format!(".rbrowser/out/screenshots/run/{index:04}.png"),
            PNG,
        );
    }
    let (images, truncated) = list(&mut host).expect("list");
    assert_eq!(images.len(), MAX_LISTED);
    assert!(truncated);
    assert_eq!(images[0], ".rbrowser/out/screenshots/run/0204.png");
}

#[test]
fn a_refused_listing_of_the_start_is_the_callers() {
    let mut host = crate::testing::FolderHost::new(rb_plugin_api::test_support::MemoryHost::new());
    let refused = last(&mut host).expect_err("no grant");
    assert_eq!(refused.code(), Some("plugins.error.capability_denied"));
}
