use std::sync::Arc;

use rb_engine::{HandleEvent, InputEvent, ScreenshotRequest, ViewHandle, ViewId};

use super::*;
use crate::engines::android::adb::tests::FakeAdb;
use crate::engines::android::fake_chrome;
use crate::engines::android::forward::remove_args;
use crate::engines::android::reach::{Notices, Reverses};
use crate::engines::android::reverse::{reverse_args, reverse_list_args, unreverse_args};
use crate::engines::android::reverse_ledger::Ledger;

fn link(chrome: &fake_chrome::FakeChrome, adb: &Arc<FakeAdb>, client: CdpClient) -> Link {
    Link {
        serial: "S1".to_owned(),
        port: chrome.port,
        adb: adb.clone(),
        client,
        reach: ReverseLink::new(
            Reverses::new(true, Ledger::at(None), |_| true),
            Notices::default(),
        ),
    }
}

async fn opened() -> (fake_chrome::FakeChrome, Arc<FakeAdb>, Arc<AndroidView>) {
    opened_at("https://example.test/").await
}

/// A view at `url` over an adb that answers the forward's removal and the reverses of 3000
/// and 5000 (the list is empty).
async fn opened_at(url: &str) -> (fake_chrome::FakeChrome, Arc<FakeAdb>, Arc<AndroidView>) {
    let chrome = fake_chrome::start().await;
    let adb = Arc::new(FakeAdb::default());
    adb.answer(&remove_args("S1", chrome.port), Ok(""));
    adb.answer(&reverse_list_args("S1"), Ok(""));
    for port in [3000, 5000] {
        adb.answer(&reverse_args("S1", port), Ok(""));
        adb.answer(&unreverse_args("S1", port), Ok(""));
    }
    let client = CdpClient::connect(&chrome.url).await.expect("connected");
    let view = AndroidView::open(ViewId(4), link(&chrome, &adb, client), url)
        .await
        .expect("opened");
    (chrome, adb, view)
}

fn called(adb: &FakeAdb, args: &[String]) -> bool {
    adb.calls().contains(&args.join(" "))
}

#[tokio::test]
async fn a_view_opens_a_tab_and_reports_its_navigations() {
    let (chrome, _adb, view) = opened().await;
    assert_eq!(
        chrome.methods(),
        [
            "Target.createTarget",
            "Target.attachToTarget",
            "Page.enable"
        ]
    );
    assert_eq!(view.url(), "https://example.test/");
    let mut events = view.events();
    view.navigate("https://example.test/next")
        .await
        .expect("navigates");
    let event = tokio::time::timeout(std::time::Duration::from_secs(5), events.recv())
        .await
        .expect("an event in time");
    assert_eq!(
        event,
        Some(HandleEvent::Navigated {
            url: "https://example.test/next".to_owned()
        })
    );
    assert_eq!(view.url(), "https://example.test/next");
}

#[tokio::test]
async fn back_steps_through_the_history_and_input_is_cdp_input() {
    let (chrome, _adb, view) = opened().await;
    view.back().await.expect("back");
    view.dispatch_input(&InputEvent::Text {
        text: "hi".to_owned(),
    })
    .await
    .expect("typed");
    let commands = crate::cdp::lock(&chrome.commands).clone();
    let entry = commands
        .iter()
        .find(|command| command["method"] == "Page.navigateToHistoryEntry")
        .expect("a history step");
    assert_eq!(entry["params"]["entryId"], 5);
    assert_eq!(entry["sessionId"], "S1");
    assert!(
        commands
            .iter()
            .any(|command| command["method"] == "Input.insertText")
    );
}

#[tokio::test]
async fn nothing_is_emulated_and_a_device_has_no_window() {
    let (_chrome, _adb, view) = opened().await;
    let report = view
        .apply_emulation(&rb_engine::Emulation::default())
        .await
        .expect("a report");
    assert!(report.unsupported().is_empty());
    assert!(view.raise().await.is_err());
    let shot = view
        .screenshot(ScreenshotRequest::default())
        .await
        .expect("a screenshot");
    assert!(!shot.png_base64.is_empty());
}

#[tokio::test]
async fn closing_closes_the_tab_and_removes_the_forward() {
    let (chrome, adb, view) = opened().await;
    view.close().await.expect("closed");
    assert!(chrome.methods().contains(&"Target.closeTarget".to_owned()));
    assert!(
        adb.calls()
            .contains(&remove_args("S1", chrome.port).join(" ")),
        "{:?}",
        adb.calls()
    );
    assert!(matches!(
        view.navigate("https://example.test/").await,
        Err(EngineError::Closed)
    ));
    view.close().await.expect("closing twice is fine");
}

/// A tab opened for a view that then failed to attach is closed again, not left on the device.
#[tokio::test]
async fn a_tab_whose_attach_fails_is_closed() {
    for refused in ["Target.attachToTarget", "Page.enable"] {
        let chrome = fake_chrome::start().await;
        chrome.refuse(refused);
        let adb = Arc::new(FakeAdb::default());
        let client = CdpClient::connect(&chrome.url).await.expect("connected");
        let opened = AndroidView::open(ViewId(4), link(&chrome, &adb, client), "").await;
        assert!(opened.is_err(), "{refused}");
        let commands = crate::cdp::lock(&chrome.commands).clone();
        let closed = commands
            .iter()
            .find(|command| command["method"] == "Target.closeTarget")
            .unwrap_or_else(|| panic!("the tab is closed after {refused}"));
        assert_eq!(closed["params"]["targetId"], "T1");
    }
}

/// RB-096-12: the first load of a loopback address reverses its port before the tab opens;
/// closing the view removes it again.
#[tokio::test]
async fn a_view_at_localhost_reverses_its_port_until_it_closes() {
    let (chrome, adb, view) = opened_at("http://localhost:3000/").await;
    assert_eq!(
        adb.calls(),
        [
            reverse_list_args("S1").join(" "),
            reverse_args("S1", 3000).join(" ")
        ],
        "before the tab"
    );
    assert_eq!(chrome.methods()[0], "Target.createTarget");
    view.close().await.expect("closed");
    assert!(
        called(&adb, &unreverse_args("S1", 3000)),
        "{:?}",
        adb.calls()
    );
}

/// Every way a URL reaches the device: a navigation (the cell's follow calls it) and a history
/// step to a loopback entry; another host runs no adb.
#[tokio::test]
async fn a_navigation_a_history_step_and_a_reload_reverse_first() {
    let (_chrome, adb, view) = opened().await;
    assert!(adb.calls().is_empty(), "{:?}", adb.calls());
    view.navigate("https://example.test/next")
        .await
        .expect("navigates");
    assert!(adb.calls().is_empty(), "{:?}", adb.calls());
    view.navigate("http://127.0.0.1:3000/")
        .await
        .expect("navigates");
    assert!(called(&adb, &reverse_args("S1", 3000)), "{:?}", adb.calls());
    // The fake's history: the entry before is http://localhost:5000/back.
    view.back().await.expect("back");
    assert!(called(&adb, &reverse_args("S1", 5000)), "{:?}", adb.calls());
    view.close().await.expect("closed");
    assert!(called(&adb, &unreverse_args("S1", 3000)));
    assert!(called(&adb, &unreverse_args("S1", 5000)));
}

/// A reload reads the list again, so a reverse that went (adb restarted) is set again.
#[tokio::test]
async fn a_reload_reads_the_reverse_list_again() {
    let (_chrome, adb, view) = opened_at("http://localhost:3000/").await;
    let lists = |adb: &FakeAdb| {
        adb.calls()
            .iter()
            .filter(|call| **call == reverse_list_args("S1").join(" "))
            .count()
    };
    assert_eq!(lists(&adb), 1);
    view.reload(false).await.expect("reloads");
    assert_eq!(lists(&adb), 2, "{:?}", adb.calls());
}

/// A view the device ended (the cable) still removes its reverses when the app closes it.
#[tokio::test]
async fn a_view_ended_by_the_device_removes_its_reverses_on_close() {
    let (_chrome, adb, view) = opened_at("http://localhost:3000/").await;
    view.closed
        .store(true, std::sync::atomic::Ordering::Release);
    view.close().await.expect("closed");
    assert!(
        called(&adb, &unreverse_args("S1", 3000)),
        "{:?}",
        adb.calls()
    );
}

/// A tab that failed to open takes its reverse with it.
#[tokio::test]
async fn a_tab_that_does_not_open_removes_its_reverse() {
    let chrome = fake_chrome::start().await;
    chrome.refuse("Target.createTarget");
    let adb = Arc::new(FakeAdb::default());
    adb.answer(&reverse_list_args("S1"), Ok(""));
    adb.answer(&reverse_args("S1", 3000), Ok(""));
    adb.answer(&unreverse_args("S1", 3000), Ok(""));
    let client = CdpClient::connect(&chrome.url).await.expect("connected");
    let opened = AndroidView::open(
        ViewId(4),
        link(&chrome, &adb, client),
        "http://localhost:3000/",
    )
    .await;
    assert!(opened.is_err());
    assert!(
        called(&adb, &unreverse_args("S1", 3000)),
        "{:?}",
        adb.calls()
    );
}

/// Code review item 6: a view closed while its reverse was being set gives the port back and
/// loads nothing.
#[tokio::test]
async fn a_view_closed_while_its_reverse_is_set_gives_it_back() {
    let (chrome, adb, view) = opened().await;
    view.closed
        .store(true, std::sync::atomic::Ordering::Release);
    let reached = view.reach("http://localhost:3000/", false).await;
    assert!(matches!(reached, Err(EngineError::Closed)), "{reached:?}");
    assert!(called(&adb, &reverse_args("S1", 3000)), "{:?}", adb.calls());
    assert!(
        called(&adb, &unreverse_args("S1", 3000)),
        "{:?}",
        adb.calls()
    );
    assert!(!chrome.methods().contains(&"Page.navigate".to_owned()));
}

/// The security review S1 through the view: a page that sends the view on to another loopback
/// port (the cell's follow navigates it) gives the port before back.
#[tokio::test]
async fn a_navigation_to_another_port_gives_the_old_one_back() {
    let (_chrome, adb, view) = opened_at("http://localhost:3000/").await;
    view.navigate("http://localhost:5000/")
        .await
        .expect("navigates");
    assert!(called(&adb, &reverse_args("S1", 5000)), "{:?}", adb.calls());
    assert!(
        called(&adb, &unreverse_args("S1", 3000)),
        "{:?}",
        adb.calls()
    );
}
