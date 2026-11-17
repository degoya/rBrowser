use std::sync::{Arc, Mutex};

use super::*;
use crate::cdp::lock;
use crate::portal::offer::set_sink;
use crate::portal::origin_map::{MAX_DEVICE_LOGGED, forget_logged};

const PAGE: &str = "http://localhost:8000";
const PORTAL: &str = "http://192.168.1.20:4000";
const VITE: &str = "http://localhost:5173";
const ASSETS: &str = "http://192.168.1.20:4100";

fn map() -> OriginMap {
    OriginMap::new(PAGE, PORTAL).with(VITE, ASSETS)
}

#[test]
fn a_miss_is_judged_against_the_shown_page() {
    let map = map();
    assert_eq!(classify(PORTAL, Some(&map)), Miss::Portal);
    assert_eq!(classify(ASSETS, Some(&map)), Miss::Portal);
    assert_eq!(classify(VITE, Some(&map)), Miss::AssetUnrewritten);
    assert_eq!(classify(PAGE, Some(&map)), Miss::PageUnrewritten);
    assert_eq!(
        classify("http://127.0.0.1:9000", Some(&map)),
        Miss::Unmapped
    );
    assert_eq!(classify("http://[::1]:5174", Some(&map)), Miss::Unmapped);
    assert_eq!(classify("http://127.0.0.1:9000", None), Miss::Unmapped);
    assert_eq!(classify("http://project.test", Some(&map)), Miss::Elsewhere);
    assert_eq!(
        classify("http://192.168.1.10:8080", Some(&map)),
        Miss::Elsewhere
    );
    // The portal forwards `http` only: a loopback `https` origin is none it could add.
    assert_eq!(
        classify("https://localhost:5173", Some(&map)),
        Miss::Elsewhere
    );
}

#[test]
fn a_socket_takes_each_origin_once_and_at_most_the_cap() {
    let mut misses = SocketMisses::default();
    assert!(misses.admit("http://localhost:5173"));
    assert!(!misses.admit("http://localhost:5173"));
    for port in 1..=MAX_SOCKET_MISSES + 4 {
        misses.admit(&format!("http://127.0.0.1:{port}"));
    }
    assert!(!misses.admit("http://127.0.0.1:60000"));
    assert_eq!(misses.len(), MAX_SOCKET_MISSES);
}

/// A loopback origin the portal does not forward reaches the offer with the page's origin; the
/// others never do. Each is logged once per start of the portal, the origin only, with what to
/// do about it.
#[test]
fn each_miss_is_logged_once_and_only_an_unmapped_loopback_one_is_offered() {
    let log = super::super::log_tests::Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);
    forget_logged();
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let sink = seen.clone();
    set_sink(Some(Arc::new(move |page: &str, origin: &str| {
        lock(&sink).push((page.to_owned(), origin.to_owned()));
    })));
    let map = map();
    for origin in [
        "http://127.0.0.1:9000",
        "http://127.0.0.1:9000",
        "http://project.test",
        "http://project.test",
        VITE,
        PORTAL,
    ] {
        device_missed(origin, None, Some(&map));
    }
    device_missed(PAGE, Some(Via::Img), Some(&map));
    device_missed("http://localhost:9001", None, None);
    set_sink(None);
    assert_eq!(
        *lock(&seen),
        vec![
            (PAGE.to_owned(), "http://127.0.0.1:9000".to_owned()),
            (PAGE.to_owned(), "http://127.0.0.1:9000".to_owned()),
        ],
        "the offer's book, not this, decides what is shown once"
    );
    let text = log.text();
    assert_eq!(
        text.matches("origin=http://127.0.0.1:9000").count(),
        1,
        "{text}"
    );
    assert!(text.contains("assetOrigins"), "{text}");
    assert_eq!(
        text.matches("origin=http://project.test").count(),
        1,
        "{text}"
    );
    assert!(text.contains("forwards only local http origins"), "{text}");
    assert_eq!(
        text.matches("origin=http://localhost:5173").count(),
        1,
        "{text}"
    );
    assert!(text.contains("an asset origin reached"), "{text}");
    assert!(
        text.contains("the page's own origin reached a portal device unrewritten"),
        "{text}"
    );
    assert!(text.contains("via=img"), "{text}");
    assert!(text.contains("origin=http://localhost:9001"), "{text}");
    assert!(
        !text.contains(PORTAL),
        "a portal origin's miss is the upstream's own: {text}"
    );
}

/// An origin a body named is not logged again when a device misses it.
#[test]
fn the_body_scan_and_the_device_log_an_origin_once_between_them() {
    let log = super::super::log_tests::Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);
    forget_logged();
    let map = map();
    crate::portal::origin_map::log_unmapped_loopback(
        b"<img src=\"http://127.0.0.1:9000/a.png\">",
        &map,
    );
    device_missed("http://127.0.0.1:9000", None, Some(&map));
    let text = log.text();
    assert_eq!(
        text.matches("origin=http://127.0.0.1:9000").count(),
        1,
        "{text}"
    );
}

/// RB-090-72 review: a device has a record of its own - it can fill that, never the body
/// scan's: the body's evidence is still logged after a device named every origin it may.
#[test]
fn a_device_cannot_silence_the_body_scan() {
    let log = super::super::log_tests::Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let _default = tracing::subscriber::set_default(subscriber);
    forget_logged();
    let map = map();
    for host in 0..MAX_DEVICE_LOGGED + 8 {
        device_missed(&format!("http://lan-{host}.test"), None, Some(&map));
    }
    crate::portal::origin_map::log_unmapped_loopback(
        b"<img src=\"http://127.0.0.1:9000/a.png\">",
        &map,
    );
    let text = log.text();
    assert_eq!(
        text.matches("a portal device failed to load").count(),
        MAX_DEVICE_LOGGED,
        "the device's record is bounded"
    );
    assert!(text.contains("origin=http://127.0.0.1:9000"), "{text}");
}
