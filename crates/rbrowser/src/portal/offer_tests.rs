use std::sync::{Arc, Mutex};

use rb_config::{ConfigFile, Environment};
use serde_json::{Value, json};

use super::*;
use crate::cdp::lock;
use crate::portal::origin_map::{OriginMap, forget_logged, log_unmapped_loopback};

fn environment(id: &str, url: &str, assets: &[&str]) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
        reload_interval_sec: None,
        asset_origins: assets.iter().map(|&origin| origin.to_owned()).collect(),
    }
}

fn offer(environment: &str, origin: &str) -> Offer {
    Offer {
        environment: environment.to_owned(),
        origin: origin.to_owned(),
    }
}

#[test]
fn the_shown_environment_is_the_first_of_the_page_origin() {
    let environments = [
        environment("staging", "https://staging.example.com", &[]),
        environment("local", "http://localhost:8000/start", &[]),
        environment("local-b", "http://localhost:8000/b", &[]),
    ];
    assert_eq!(
        environment_for(&environments, "http://localhost:8000").map(|env| env.id.as_str()),
        Some("local")
    );
    assert!(environment_for(&environments, "http://localhost:9").is_none());
}

/// Exactly the origin the page named, canonical; another loopback name of a listed port is
/// another origin (the dev server may listen on one of them only).
#[test]
fn the_origin_to_add_is_exactly_the_one_named() {
    let environments = vec![environment(
        "local",
        "http://localhost:8000",
        &["http://127.0.0.1:5173/"],
    )];
    assert_eq!(
        origin_to_add(&environments, "local", "http://LOCALHOST:5173/"),
        Ok(Some("http://localhost:5173".to_owned()))
    );
    assert_eq!(
        origin_to_add(&environments, "local", "http://127.0.0.1:5173"),
        Ok(None)
    );
}

#[test]
fn only_a_bare_loopback_origin_of_a_known_environment_is_added() {
    let environments = vec![environment("local", "http://localhost:8000", &[])];
    for origin in [
        "http://cdn.example.com:5173",
        "https://localhost:5173",
        "http://localhost:5173/path",
        "http://user@localhost:5173",
        "javascript:alert(1)",
        "",
    ] {
        assert_eq!(
            origin_to_add(&environments, "local", origin),
            Err(AddError::NotLoopback),
            "{origin}"
        );
    }
    assert_eq!(
        origin_to_add(&environments, "gone", "http://localhost:5173"),
        Err(AddError::UnknownEnvironment)
    );
}

/// The list in effect comes from `config.local.json` when its entry sets `assetOrigins` (a list
/// there replaces the one in `config.json`); else from `config.json`'s entry; a local-only
/// environment's entry is in `config.local.json`.
#[test]
fn the_target_is_the_file_the_list_comes_from() {
    let project = json!({ "environments": [
        { "id": "local", "url": "http://localhost:8000", "assetOrigins": ["http://localhost:5174"] },
        { "id": "staging", "url": "https://staging.example.com" }
    ] });
    let local = json!({ "environments": [
        { "id": "local", "assetOrigins": ["http://127.0.0.1:5176"] },
        { "id": "staging", "color": "#ff0000" },
        { "id": "mine", "url": "http://localhost:9000" }
    ] });
    assert_eq!(
        target_file(&project, Some(&local), "local"),
        Some(ConfigFile::Local)
    );
    assert_eq!(
        target_file(&project, Some(&local), "staging"),
        Some(ConfigFile::Project)
    );
    assert_eq!(
        target_file(&project, Some(&local), "mine"),
        Some(ConfigFile::Local)
    );
    assert_eq!(
        target_file(&project, None, "local"),
        Some(ConfigFile::Project)
    );
    assert_eq!(target_file(&project, Some(&local), "gone"), None);
    assert_eq!(
        target_file(&json!({ "version": 1 }), Some(&json!({})), "local"),
        None
    );
}

/// Only the one environment's list changes; every other entry, field and key stays.
#[test]
fn only_the_one_list_changes_in_the_file() {
    let document = json!({
        "version": 1,
        "environments": [
            { "id": "local", "url": "http://localhost:8000", "color": "#00ff00",
              "assetOrigins": ["http://localhost:5174"] },
            { "id": "staging", "url": "https://staging.example.com" }
        ],
        "unknownPluginSection": { "x": 1 }
    });
    let mut expected = document.clone();
    expected["environments"][0]["assetOrigins"] =
        json!(["http://localhost:5174", "http://localhost:5173"]);
    assert_eq!(
        with_file_origin(&document, "local", "http://localhost:5173"),
        Some(expected)
    );
    let mut made = document.clone();
    made["environments"][1]["assetOrigins"] = json!(["http://localhost:5173"]);
    assert_eq!(
        with_file_origin(&document, "staging", "http://localhost:5173"),
        Some(made)
    );
    assert_eq!(
        with_file_origin(&document, "gone", "http://localhost:5173"),
        None
    );
    assert_eq!(
        with_file_origin(&json!({ "version": 1 }), "local", "http://localhost:5173"),
        None
    );
}

/// The button's arguments name both; without arguments the command takes the oldest offer that
/// waits.
#[test]
fn the_arguments_name_the_origin_and_the_environment_or_the_oldest_offer() {
    let first = offer("local", "http://localhost:5173");
    assert_eq!(
        first.args(),
        json!({ "environment": "local", "origin": "http://localhost:5173" })
    );
    assert_eq!(Offer::from_args(&first.args(), None), Ok(first.clone()));
    assert_eq!(
        Offer::from_args(&Value::Null, Some(first.clone())),
        Ok(first.clone())
    );
    assert_eq!(
        Offer::from_args(&json!({}), Some(first.clone())),
        Ok(first.clone())
    );
    assert!(Offer::from_args(&Value::Null, None).is_err());
    assert!(Offer::from_args(&json!({ "origin": "http://localhost:5173" }), None).is_err());
    assert!(Offer::from_args(&json!({ "origin": 1, "environment": "local" }), None).is_err());
}

/// Each origin is offered once while one page origin is shown; another page origin (another
/// environment) and a cleared book (a start of the portal) offer it again. The offers wait in
/// order; the oldest is the one taken without arguments.
#[test]
fn the_book_offers_once_per_shown_environment_and_keeps_the_order() {
    let mut book = OfferBook::new();
    assert!(book.note("http://localhost:8000", "http://localhost:5173"));
    assert!(!book.note("http://localhost:8000", "http://localhost:5173"));
    assert!(book.note("http://localhost:8000", "http://127.0.0.1:5173"));
    assert!(book.note("http://localhost:9000", "http://localhost:5173"));
    assert!(book.note("http://localhost:8000", "http://localhost:5173"));
    book.clear();
    assert!(book.note("http://localhost:8000", "http://localhost:5173"));

    let a = offer("local", "http://localhost:5173");
    let b = offer("local", "http://localhost:5174");
    book.wait(a.clone());
    book.wait(b.clone());
    book.wait(a.clone());
    assert_eq!(book.oldest(), Some(a.clone()));
    book.taken(&a);
    assert_eq!(book.oldest(), Some(b.clone()));
    book.taken(&b);
    assert_eq!(book.oldest(), None);
}

/// An MCP client cannot run it: only a person's click writes the project config.
#[test]
fn mcp_cannot_run_the_command() {
    assert!(refuse_mcp(ADD_ASSET_ORIGIN).is_err());
    assert!(refuse_mcp("portal.disconnect").is_ok());
    assert!(refuse_mcp("view.reload").is_ok());
}

/// The portal hands every unmapped loopback origin a body names to the sink, exactly as named,
/// with the page's origin (the book decides what to show); nothing without a sink.
#[test]
fn every_unmapped_loopback_origin_reaches_the_sink_as_named() {
    forget_logged();
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::default();
    let sink = seen.clone();
    set_sink(Some(Arc::new(move |page: &str, origin: &str| {
        lock(&sink).push((page.to_owned(), origin.to_owned()));
    })));
    let map = OriginMap::new("http://localhost:8000", "http://192.168.1.20:4000");
    log_unmapped_loopback(
        b"<img src=\"http://localhost:41731/a.png\"><img src=\"http://127.0.0.1:41731/b.png\">",
        &map,
    );
    log_unmapped_loopback(b"<img src=\"http://localhost:41731/c.png\">", &map);
    set_sink(None);
    log_unmapped_loopback(b"<img src=\"http://127.0.0.1:41733/e.png\">", &map);
    let page = "http://localhost:8000".to_owned();
    assert_eq!(
        *lock(&seen),
        vec![
            (page.clone(), "http://localhost:41731".to_owned()),
            (page.clone(), "http://127.0.0.1:41731".to_owned()),
            (page, "http://localhost:41731".to_owned()),
        ]
    );
}

/// RB-090-72 review: no click may put one of rBrowser's own listeners into the LAN - the MCP
/// server, the CDP proxy, the portal's own listeners - whether a body names it or a device
/// reports it; any other loopback origin is offered as before.
#[test]
fn an_origin_on_a_port_of_rbrowser_s_own_listeners_is_never_offered() {
    forget_logged();
    let seen: Arc<Mutex<Vec<String>>> = Arc::default();
    let sink = seen.clone();
    set_sink(Some(Arc::new(move |_: &str, origin: &str| {
        lock(&sink).push(origin.to_owned());
    })));
    // The MCP server on 7333, the CDP proxy on 9333.
    set_own_ports(Some(Arc::new(|| vec![7333, 9333])));
    let map = OriginMap::new("http://localhost:8000", "http://192.168.1.20:4000")
        .with("http://localhost:5173", "http://192.168.1.20:4100");
    log_unmapped_loopback(
        b"<script src=\"http://127.0.0.1:7333/mcp\"></script>\
          <img src=\"http://localhost:9333/json\"><img src=\"http://127.0.0.1:4000/x\">\
          <img src=\"http://127.0.0.1:4100/y\"><img src=\"http://localhost:41731/fine.png\">",
        &map,
    );
    for origin in [
        "http://[::1]:7333",
        "http://localhost:9333",
        "http://localhost:4100",
        "http://127.0.0.1:41732",
    ] {
        crate::portal::misses::device_missed(origin, None, Some(&map));
    }
    set_sink(None);
    set_own_ports(None);
    assert_eq!(
        *lock(&seen),
        vec![
            "http://localhost:41731".to_owned(),
            "http://127.0.0.1:41732".to_owned()
        ]
    );
}
