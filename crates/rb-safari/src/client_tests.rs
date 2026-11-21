use serde_json::json;

use super::*;
use crate::testing::{FAKE_PNG, FakeSafari};

async fn session(fake: &FakeSafari) -> Session {
    DriverClient::new(fake.port())
        .new_session(json!({ "browserName": "safari" }))
        .await
        .expect("a session")
}

#[tokio::test]
async fn the_driver_reports_ready_and_opens_a_session() {
    let fake = FakeSafari::start().await;
    let driver = DriverClient::new(fake.port());
    assert!(driver.ready().await.expect("status"));
    let session = session(&fake).await;
    assert!(session.id().starts_with("fake-session-"));
    assert_eq!(session.browser_version(), Some("26.1"));
    let created = fake.requests_to("/session");
    assert_eq!(
        created[0].body,
        json!({ "capabilities": { "alwaysMatch": { "browserName": "safari" } } })
    );
}

#[tokio::test]
async fn navigation_and_history_go_to_the_session_s_paths() {
    let fake = FakeSafari::start().await;
    let session = session(&fake).await;
    session.navigate("http://localhost/a").await.expect("a");
    session.navigate("http://localhost/b").await.expect("b");
    assert_eq!(
        session.current_url().await.expect("url"),
        "http://localhost/b"
    );
    session.back().await.expect("back");
    assert_eq!(
        fake.url_of(session.id()).as_deref(),
        Some("http://localhost/a")
    );
    session.forward().await.expect("forward");
    session.refresh().await.expect("refresh");
    let paths: Vec<String> = fake
        .requests()
        .into_iter()
        .map(|request| format!("{} {}", request.method, request.path))
        .collect();
    let id = session.id();
    for expected in [
        format!("POST /session/{id}/url"),
        format!("GET /session/{id}/url"),
        format!("POST /session/{id}/back"),
        format!("POST /session/{id}/forward"),
        format!("POST /session/{id}/refresh"),
    ] {
        assert!(paths.contains(&expected), "{expected} in {paths:?}");
    }
}

#[tokio::test]
async fn scripts_actions_window_and_screenshots_reach_the_driver() {
    let fake = FakeSafari::start().await;
    let session = session(&fake).await;
    let probe = session
        .execute(crate::emulation::VIEWPORT_PROBE, Vec::new())
        .await
        .expect("probe");
    assert_eq!(probe, json!([0, 74, 1024, 768]));
    session
        .perform_actions(vec![
            json!({ "type": "key", "id": "keyboard", "actions": [] }),
        ])
        .await
        .expect("actions");
    session.release_actions().await.expect("released");
    session
        .set_window_rect(json!({ "width": 375, "height": 741 }))
        .await
        .expect("sized");
    assert_eq!(fake.rect_of(session.id()), Some((0, 0, 375, 741)));
    session.focus_window().await.expect("focused");
    assert_eq!(session.screenshot().await.expect("png"), FAKE_PNG);
    let html = session.find_css("html").await.expect("html");
    assert_eq!(
        session.element_screenshot(&html).await.expect("png"),
        FAKE_PNG
    );
    let actions = fake.requests_to("/actions");
    assert_eq!(actions[0].method, "POST");
    assert_eq!(actions[1].method, "DELETE");
}

#[tokio::test]
async fn a_missing_element_is_webdriver_s_error() {
    let fake = FakeSafari::start().await;
    let session = session(&fake).await;
    fake.hide("#gone");
    let error = session.find_css("#gone").await.expect_err("missing");
    assert!(
        matches!(&error, SafariError::Command { error, .. } if error == "no such element"),
        "{error:?}"
    );
    assert!(session.element_screenshot("../x").await.is_err());
}

#[tokio::test]
async fn refused_sessions_say_why() {
    let fake = FakeSafari::start().await;
    fake.refuse_sessions(
        "Could not create a session: You must enable the 'Allow Remote Automation' option in Safari's Develop menu to control Safari via WebDriver.",
    );
    let driver = DriverClient::new(fake.port());
    assert_eq!(
        driver.new_session(json!({})).await,
        Err(SafariError::NotEnabled)
    );

    let busy = FakeSafari::start().await;
    busy.max_sessions(1);
    let driver = DriverClient::new(busy.port());
    driver.new_session(json!({})).await.expect("the first");
    assert_eq!(
        driver.new_session(json!({})).await,
        Err(SafariError::SessionBusy)
    );
}

#[tokio::test]
async fn deleting_the_session_closes_its_window() {
    let fake = FakeSafari::start().await;
    let session = session(&fake).await;
    assert_eq!(fake.sessions(), 1);
    session.delete().await.expect("deleted");
    assert_eq!(fake.sessions(), 0);
    let error = session.current_url().await.expect_err("gone");
    assert!(matches!(&error, SafariError::Command { error, .. } if error == "invalid session id"));
}

#[tokio::test]
async fn a_stopped_driver_is_gone() {
    let fake = FakeSafari::start().await;
    let session = session(&fake).await;
    fake.stop();
    // The listener ends with its task; the next request finds nobody.
    let mut gone = false;
    for _ in 0..50 {
        match session.current_url().await {
            Err(error) if error.driver_gone() => {
                gone = true;
                break;
            }
            _ => tokio::task::yield_now().await,
        }
    }
    assert!(gone, "the driver should be gone");
}
