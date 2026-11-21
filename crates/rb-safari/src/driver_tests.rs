use std::path::Path;
use std::time::Duration;

use serde_json::json;

use super::*;
use crate::testing::FakeSafari;

#[test]
fn the_driver_listens_on_the_port_it_is_given() {
    assert_eq!(arguments(4711), ["--port", "4711"]);
}

#[test]
fn each_edition_has_its_driver_browser_and_bundle() {
    assert_eq!(Edition::from_setting(false), Edition::Safari);
    assert_eq!(Edition::from_setting(true), Edition::TechnologyPreview);
    assert_eq!(Edition::Safari.driver(), Path::new("/usr/bin/safaridriver"));
    assert!(
        Edition::TechnologyPreview
            .driver()
            .starts_with("/Applications/Safari Technology Preview.app")
    );
    assert_eq!(
        capabilities(Edition::Safari),
        json!({ "browserName": "safari" })
    );
    assert_eq!(
        capabilities(Edition::TechnologyPreview),
        json!({ "browserName": "Safari Technology Preview" })
    );
    assert!(Edition::Safari.bundle().ends_with("Safari.app"));
}

#[test]
fn a_free_port_is_on_loopback() {
    let port = free_port().expect("a free port");
    assert_ne!(port, 0);
}

#[tokio::test]
async fn a_driver_that_cannot_start_says_so() {
    let missing = Path::new("/nonexistent/rbrowser-test/safaridriver");
    let Err(SafariError::Launch(reason)) = start(missing, Duration::from_secs(1)).await else {
        panic!("a launch error");
    };
    assert!(reason.contains("safaridriver"), "{reason}");
}

#[tokio::test]
async fn the_real_launcher_refuses_a_driver_that_is_not_there() {
    // Outside macOS neither driver exists; on a Mac Technology Preview may be missing too.
    if Edition::TechnologyPreview.driver().is_file() {
        return;
    }
    let Err(SafariError::Launch(reason)) = DriverLaunch.launch(Edition::TechnologyPreview).await
    else {
        panic!("a launch error");
    };
    assert!(!reason.is_empty());
}

#[tokio::test]
async fn an_attached_driver_is_used_as_it_is() {
    let fake = FakeSafari::start().await;
    let launched = Attached(fake.port())
        .launch(Edition::Safari)
        .await
        .expect("attached");
    assert_eq!(launched.client.port(), fake.port());
    assert!(launched.process.is_none());
    assert!(launched.client.ready().await.expect("status"));
}
