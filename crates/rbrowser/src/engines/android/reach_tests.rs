use std::sync::Arc;

use super::*;
use crate::engines::android::adb::tests::{FakeAdb, Log};
use crate::engines::android::reverse::{reverse_args, reverse_list_args, unreverse_args};
use crate::engines::android::reverse_ledger::{Entry, FOLDER_NAME, device_key};

const SERIAL: &str = "R58N123ABC";
const URL: &str = "http://localhost:3000/app";

fn fake(listed: &str) -> Arc<FakeAdb> {
    let adb = Arc::new(FakeAdb::default());
    adb.answer(&reverse_list_args(SERIAL), Ok(listed));
    for port in [3000, 4000, 5000] {
        adb.answer(&reverse_args(SERIAL, port), Ok(""));
        adb.answer(&unreverse_args(SERIAL, port), Ok(""));
    }
    adb
}

fn runner(adb: &Arc<FakeAdb>) -> Arc<dyn AdbRun> {
    adb.clone()
}

fn reverses(allowed: bool) -> Arc<Reverses> {
    Reverses::new(allowed, Ledger::at(None), |_| true)
}

fn count(adb: &FakeAdb, args: &[String]) -> usize {
    let line = args.join(" ");
    adb.calls().iter().filter(|call| **call == line).count()
}

/// Before the page loads: the list is read, then exactly that port is reversed; a second
/// load of the same port runs adb no more.
#[tokio::test]
async fn a_loopback_address_gets_its_port_reversed_once() {
    let adb = fake("");
    let reverses = reverses(true);
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    assert_eq!(reach, Reach::Set(3000));
    assert_eq!(
        adb.calls(),
        [
            reverse_list_args(SERIAL).join(" "),
            reverse_args(SERIAL, 3000).join(" ")
        ]
    );
    let again = reverses
        .prepare(
            &runner(&adb),
            SERIAL,
            1,
            "http://localhost:3000/other",
            false,
        )
        .await;
    assert_eq!(again, Reach::Stands(3000));
    assert_eq!(adb.calls().len(), 2, "{:?}", adb.calls());
}

#[tokio::test]
async fn another_host_runs_no_adb() {
    let adb = fake("");
    let reverses = reverses(true);
    for url in [
        "https://example.test/",
        "http://192.168.1.20:3000/",
        "about:blank",
    ] {
        let reach = reverses.prepare(&runner(&adb), SERIAL, 1, url, false).await;
        assert_eq!(reach, Reach::Remote, "{url}");
    }
    assert!(adb.calls().is_empty(), "{:?}", adb.calls());
}

/// A reverse someone else set on the port is neither replaced nor removed.
#[tokio::test]
async fn a_foreign_reverse_is_left_alone() {
    let adb = fake("UsbFfs tcp:3000 tcp:9000\n");
    let reverses = reverses(true);
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    assert_eq!(reach, Reach::Foreign(3000));
    reverses.release(SERIAL, 1).await;
    reverses.clear().await;
    assert_eq!(count(&adb, &reverse_args(SERIAL, 3000)), 0);
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 0);
}

/// Closing the last view that needs a port removes its reverse; a second view on the same
/// device and port keeps it until it closes too.
#[tokio::test]
async fn the_last_view_that_needs_a_port_removes_it() {
    let adb = fake("");
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    adb.answer(&reverse_list_args(SERIAL), Ok("x tcp:3000 tcp:3000\n"));
    let second = reverses.prepare(&runner(&adb), SERIAL, 2, URL, false).await;
    assert_eq!(second, Reach::Stands(3000));
    reverses.release(SERIAL, 1).await;
    assert_eq!(
        count(&adb, &unreverse_args(SERIAL, 3000)),
        0,
        "view 2 needs it"
    );
    reverses.release(SERIAL, 2).await;
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1);
    reverses.release(SERIAL, 2).await;
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1, "once");
}

/// Android or the setting switched off, or the app quitting: every reverse rBrowser set goes.
#[tokio::test]
async fn clearing_removes_every_own_reverse() {
    let adb = fake("");
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    reverses
        .prepare(&runner(&adb), SERIAL, 2, "http://127.0.0.1:4000/", false)
        .await;
    reverses.clear().await;
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1);
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 4000)), 1);
    assert!(!reverses.holds(SERIAL, 3000));
    reverses.clear().await;
    assert_eq!(
        count(&adb, &unreverse_args(SERIAL, 3000)),
        1,
        "nothing left"
    );
}

/// Off: nothing runs, and the answer says the device cannot reach the port.
#[tokio::test]
async fn switched_off_nothing_is_reversed() {
    let adb = fake("");
    let reverses = reverses(false);
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    assert_eq!(reach, Reach::Off(3000));
    assert!(adb.calls().is_empty(), "{:?}", adb.calls());
    reverses.set_allowed(true);
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    assert_eq!(reach, Reach::Set(3000));
}

/// A reload checks the list again: a reverse that went (adb restarted) is set again.
#[tokio::test]
async fn a_reload_sets_a_reverse_that_went_again() {
    let adb = fake("");
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, true).await;
    assert_eq!(reach, Reach::Set(3000));
    assert_eq!(count(&adb, &reverse_args(SERIAL, 3000)), 2);
    adb.answer(&reverse_list_args(SERIAL), Ok("x tcp:3000 tcp:3000\n"));
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, true).await;
    assert_eq!(reach, Reach::Stands(3000));
}

/// RB-096-10: the debug log names the device by four characters of its serial, even when
/// adb's own error text names the whole serial.
#[tokio::test]
async fn the_log_never_names_the_whole_serial() {
    let log = Log::default();
    let _guard = tracing::subscriber::set_default(log.subscriber());
    let adb = Arc::new(FakeAdb::default());
    adb.answer(&reverse_list_args(SERIAL), Ok(""));
    adb.answer(
        &reverse_args(SERIAL, 3000),
        Err(AdbError::Failed(format!(
            "error: device '{SERIAL}' not found"
        ))),
    );
    let reverses = reverses(true);
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    assert_eq!(reach, Reach::Failed(3000));
    let text = log.text();
    assert!(text.contains("R58N\u{2026}"), "{text}");
    assert!(!text.contains(SERIAL), "{text}");
}

/// The view's side: an address the device cannot reach (the setting off) is told once per
/// port, with the setting linked.
#[tokio::test]
async fn switched_off_the_view_tells_once_per_port_with_the_setting() {
    let told = Arc::new(std::sync::Mutex::new(Vec::<CoreToShell>::new()));
    let notices = Notices::default();
    let sink = told.clone();
    notices.set(Arc::new(move |message| {
        crate::cdp::lock(&sink).push(message)
    }));
    let link = ReverseLink::new(reverses(false), notices);
    let adb = fake("");
    for url in [URL, "http://localhost:3000/b", "http://127.0.0.1:4000/"] {
        link.prepare(&runner(&adb), SERIAL, url, false).await;
    }
    let told = crate::cdp::lock(&told).clone();
    assert_eq!(told.len(), 2, "{told:?}");
    let CoreToShell::Notification(first) = &told[0] else {
        panic!("a notification");
    };
    assert_eq!(first.message_key, OFF_KEY);
    assert_eq!(first.settings_key.as_deref(), Some(SETTING));
    assert_eq!(
        first.args.get("address").map(String::as_str),
        Some("localhost:3000")
    );
}

/// A run that ended without cleaning up: its entries are dropped from the ledger at the start
/// and its reverses are never removed - a `tcp:P tcp:P` listed now may be another tool's (code
/// review w33, item 4); a running rBrowser's entries stay.
#[tokio::test]
async fn an_orphan_of_a_run_before_is_forgotten_not_removed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let folder = dir.path().join(FOLDER_NAME);
    let gone = std::process::id().wrapping_add(1);
    let running = std::process::id().wrapping_add(2);
    let device = device_key(SERIAL);
    std::fs::create_dir_all(&folder).expect("folder");
    for (pid, port) in [(gone, 4000), (running, 5000)] {
        let text = serde_json::json!({ "version": 1,
            "reverses": [{ "device": device, "port": port }] });
        std::fs::write(folder.join(format!("{pid}.json")), text.to_string()).expect("written");
    }
    let adb = fake("x tcp:4000 tcp:4000\nx tcp:5000 tcp:5000\n");
    let reverses = Reverses::new(true, Ledger::at(Some(folder.clone())), move |pid| {
        pid == running
    });
    let reach = reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    assert_eq!(reach, Reach::Set(3000));
    assert!(
        !adb.calls().iter().any(|call| call.contains("--remove")),
        "{:?}",
        adb.calls()
    );
    let mut entries = Ledger::at(Some(folder)).entries();
    entries.sort();
    let mut expected = vec![
        Entry::new(&device, 3000, std::process::id()),
        Entry::new(&device, 5000, running),
    ];
    expected.sort();
    assert_eq!(entries, expected);
}

/// A reverse adb did not remove (the device busy) is not tried again on the next use: by then
/// it may be someone else's.
#[tokio::test]
async fn a_reverse_adb_did_not_remove_is_not_tried_again() {
    let adb = Arc::new(FakeAdb::default());
    adb.answer(&reverse_list_args(SERIAL), Ok(""));
    adb.answer(&reverse_args(SERIAL, 3000), Ok(""));
    adb.answer(&reverse_args(SERIAL, 4000), Ok(""));
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    reverses.release(SERIAL, 1).await;
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1);
    adb.answer(&reverse_list_args(SERIAL), Ok("x tcp:3000 tcp:3000\n"));
    let reach = reverses
        .prepare(&runner(&adb), SERIAL, 2, "http://localhost:4000/", false)
        .await;
    assert_eq!(reach, Reach::Set(4000));
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1, "not again");
}

/// The security review S1: a view holds only the port of the address it loads now. A page that
/// sends it on to `localhost:2375` takes 3000 with it; a remote address leaves every port.
#[tokio::test]
async fn a_view_holds_only_the_port_of_its_current_address() {
    let adb = fake("");
    adb.answer(&reverse_args(SERIAL, 2375), Ok(""));
    adb.answer(&unreverse_args(SERIAL, 2375), Ok(""));
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    let reach = reverses
        .prepare(&runner(&adb), SERIAL, 1, "http://localhost:2375/", false)
        .await;
    assert_eq!(reach, Reach::Set(2375));
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1);
    assert!(!reverses.holds(SERIAL, 3000));
    let reach = reverses
        .prepare(&runner(&adb), SERIAL, 1, "https://example.test/", false)
        .await;
    assert_eq!(reach, Reach::Remote);
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 2375)), 1);
    assert!(!reverses.holds(SERIAL, 2375));
}

/// A port another view still needs stays when one view moves on.
#[tokio::test]
async fn moving_on_respects_the_other_views_of_a_port() {
    let adb = fake("");
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    reverses.prepare(&runner(&adb), SERIAL, 2, URL, false).await;
    reverses
        .prepare(&runner(&adb), SERIAL, 1, "http://localhost:4000/", false)
        .await;
    assert_eq!(
        count(&adb, &unreverse_args(SERIAL, 3000)),
        0,
        "view 2 needs it"
    );
    assert!(reverses.holds(SERIAL, 3000));
    reverses
        .prepare(&runner(&adb), SERIAL, 2, "about:blank", false)
        .await;
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1);
}

/// Code review item 5: a prepare that waited for the device while the setting went off and
/// `clear` ran sets nothing.
#[tokio::test]
async fn a_prepare_waiting_while_clear_runs_sets_nothing() {
    let adb = fake("");
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    let device = reverses.known(SERIAL).expect("known");
    let guard = device.lock().await;
    let waiting = {
        let reverses = reverses.clone();
        let adb = runner(&adb);
        tokio::spawn(async move {
            reverses
                .prepare(&adb, SERIAL, 2, "http://localhost:4000/", false)
                .await
        })
    };
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    reverses.set_allowed(false);
    let clearing = {
        let reverses = reverses.clone();
        tokio::spawn(async move { reverses.clear().await })
    };
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    drop(guard);
    assert_eq!(waiting.await.expect("ran"), Reach::Off(4000));
    clearing.await.expect("ran");
    assert_eq!(
        count(&adb, &reverse_args(SERIAL, 4000)),
        0,
        "{:?}",
        adb.calls()
    );
    assert_eq!(count(&adb, &unreverse_args(SERIAL, 3000)), 1);
}

/// Code review item 7: each device has its own lock - a device busy with adb stalls no other.
#[tokio::test]
async fn a_busy_device_stalls_no_other() {
    const OTHER: &str = "ZY22ABCDEF";
    let adb = fake("");
    adb.answer(&reverse_list_args(OTHER), Ok(""));
    adb.answer(&reverse_args(OTHER, 3000), Ok(""));
    let reverses = reverses(true);
    reverses.prepare(&runner(&adb), SERIAL, 1, URL, false).await;
    let device = reverses.known(SERIAL).expect("known");
    let _busy = device.lock().await;
    // A give-up deadline, no measure of speed: without a lock per device this never ends.
    let other = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        reverses.prepare(&runner(&adb), OTHER, 2, URL, false),
    )
    .await
    .expect("the other device is not held up");
    assert_eq!(other, Reach::Set(3000));
}
