use super::*;

fn line(value: serde_json::Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&value).expect("encodes");
    bytes.push(b'\n');
    bytes
}

const NONCE: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

#[test]
fn a_nonce_is_64_hex_digits_and_new_each_time() {
    let (a, b) = (new_nonce().expect("random"), new_nonce().expect("random"));
    assert_eq!(a.len(), 64);
    assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    assert_ne!(a, b);
}

#[test]
fn nonces_are_compared_whole() {
    assert!(nonce_matches(NONCE, NONCE));
    assert!(!nonce_matches(NONCE, &NONCE[..63]));
    assert!(!nonce_matches(NONCE, &NONCE.replace('0', "1")));
    assert!(!nonce_matches(NONCE, ""));
}

#[test]
fn an_open_request_round_trips_and_its_urls_are_completed() {
    let envelope = Envelope::new(
        NONCE,
        Request::Open {
            urls: vec!["localhost:5173/a".to_owned()],
            background: true,
        },
    );
    let bytes = encode(&envelope);
    assert_eq!(bytes.last(), Some(&b'\n'));
    assert_eq!(
        check(&bytes, NONCE),
        Ok(Request::Open {
            urls: vec!["http://localhost:5173/a".to_owned()],
            background: true,
        })
    );
}

#[test]
fn a_reload_request_is_taken() {
    let bytes = line(serde_json::json!({
        "version": 1, "nonce": NONCE, "request": { "kind": "reload", "ignoreCache": true }
    }));
    assert_eq!(
        check(&bytes, NONCE),
        Ok(Request::Reload { ignore_cache: true })
    );
}

#[test]
fn a_wrong_nonce_is_refused() {
    let bytes = encode(&Envelope::new(
        &NONCE.replace('a', "b"),
        Request::Reload {
            ignore_cache: false,
        },
    ));
    assert_eq!(check(&bytes, NONCE), Err(Refusal::BadNonce));
    let bytes = line(serde_json::json!({
        "version": 1, "nonce": "", "request": { "kind": "reload" }
    }));
    assert_eq!(check(&bytes, NONCE), Err(Refusal::BadNonce));
}

#[test]
fn unknown_requests_and_fields_are_refused() {
    for value in [
        serde_json::json!({ "version": 1, "nonce": NONCE, "request": { "kind": "exec", "cmd": "calc" } }),
        serde_json::json!({ "version": 1, "nonce": NONCE, "request": { "kind": "reload", "script": "x" } }),
        serde_json::json!({ "version": 1, "nonce": NONCE, "request": { "kind": "open" }, "extra": 1 }),
        serde_json::json!({ "version": 1, "nonce": NONCE }),
        serde_json::json!("open"),
    ] {
        assert_eq!(
            check(&line(value.clone()), NONCE),
            Err(Refusal::BadRequest),
            "{value}"
        );
    }
    assert_eq!(check(b"not json\n", NONCE), Err(Refusal::BadRequest));
    assert_eq!(check(&[0xff, 0xfe, b'\n'], NONCE), Err(Refusal::BadRequest));
}

#[test]
fn another_protocol_version_is_refused() {
    let bytes = line(serde_json::json!({
        "version": 2, "nonce": NONCE, "request": { "kind": "reload" }
    }));
    assert_eq!(check(&bytes, NONCE), Err(Refusal::BadVersion));
}

#[test]
fn urls_outside_the_allowed_schemes_or_limits_are_refused() {
    for url in [
        "javascript:alert(1)",
        "data:text/html,<script>",
        "chrome://settings",
        "rbrowser://ui/index.html",
        "http://a b",
        "",
    ] {
        let bytes = encode(&Envelope::new(
            NONCE,
            Request::Open {
                urls: vec![url.to_owned()],
                background: false,
            },
        ));
        assert_eq!(check(&bytes, NONCE), Err(Refusal::BadUrl), "{url}");
    }
    let long = format!("http://localhost/{}", "a".repeat(MAX_URL_LEN));
    assert!(open_url(&long).is_err());
    let many = vec!["http://localhost/".to_owned(); MAX_URLS + 1];
    let bytes = encode(&Envelope::new(
        NONCE,
        Request::Open {
            urls: many,
            background: false,
        },
    ));
    assert_eq!(check(&bytes, NONCE), Err(Refusal::BadRequest));
}

#[test]
fn file_and_https_urls_are_taken() {
    assert_eq!(
        open_url("file:///C:/site/index.html").as_deref(),
        Ok("file:///C:/site/index.html")
    );
    assert_eq!(
        open_url("example.com").as_deref(),
        Ok("https://example.com")
    );
}

#[test]
fn replies_round_trip() {
    for reply in [Reply::ok(), Reply::refused(Refusal::BadNonce)] {
        let bytes = encode_reply(&reply);
        assert_eq!(decode_reply(&bytes), Some(reply));
    }
    assert_eq!(decode_reply(b"{\"ok\":true,\"x\":1}\n"), None);
}

#[test]
fn the_nonce_never_appears_in_debug_output() {
    let envelope = Envelope::new(
        NONCE,
        Request::Reload {
            ignore_cache: false,
        },
    );
    assert!(!format!("{envelope:?}").contains(NONCE));
}

fn shots_dir() -> String {
    std::env::temp_dir()
        .join("shots")
        .to_str()
        .expect("a Unicode temp folder")
        .to_owned()
}

fn screenshot(args: ScreenshotArgs, views: &[&str], out: Option<String>) -> Request {
    Request::Screenshot {
        args,
        views: views.iter().map(|view| (*view).to_owned()).collect(),
        out,
    }
}

fn full_page() -> ScreenshotArgs {
    ScreenshotArgs {
        target: Some(rb_engine::Target::All),
        mode: Some(crate::capture::CaptureMode::FullPage),
        delay_ms: Some(5000),
        save: Some(true),
        ..ScreenshotArgs::default()
    }
}

#[test]
fn a_screenshot_request_round_trips() {
    let request = screenshot(full_page(), &["mobile", "3"], Some(shots_dir()));
    let bytes = encode(&Envelope::new(NONCE, request.clone()));
    assert_eq!(check(&bytes, NONCE), Ok(request));
    let bare = line(serde_json::json!({
        "version": 1, "nonce": NONCE, "request": { "kind": "screenshot", "args": {} }
    }));
    assert_eq!(
        check(&bare, NONCE),
        Ok(screenshot(ScreenshotArgs::default(), &[], None))
    );
}

#[test]
fn screenshot_requests_out_of_bounds_are_refused() {
    let parent = format!("{}{}..{}x", shots_dir(), std::path::MAIN_SEPARATOR, '/');
    let refused = [
        screenshot(
            ScreenshotArgs {
                delay_ms: Some(MAX_DELAY_MS + 1),
                ..full_page()
            },
            &[],
            None,
        ),
        screenshot(
            ScreenshotArgs {
                mode: Some(crate::capture::CaptureMode::Element),
                selector: Some("a".repeat(MAX_SELECTOR_LEN + 1)),
                ..ScreenshotArgs::default()
            },
            &[],
            None,
        ),
        screenshot(
            ScreenshotArgs {
                save: Some(false),
                ..full_page()
            },
            &[],
            None,
        ),
        screenshot(full_page(), &[], Some("shots".to_owned())),
        screenshot(full_page(), &[], Some(parent)),
        screenshot(full_page(), &[], Some(format!("{}\u{7}", shots_dir()))),
        screenshot(full_page(), &[""], None),
        screenshot(full_page(), &["\u{1b}[2J"], None),
        screenshot(
            full_page(),
            &["m".repeat(MAX_VIEW_NAME_LEN + 1).as_str()],
            None,
        ),
        screenshot(full_page(), &["mobile"; MAX_VIEW_NAMES + 1], None),
    ];
    for request in refused {
        let bytes = encode(&Envelope::new(NONCE, request.clone()));
        assert_eq!(
            check(&bytes, NONCE),
            Err(Refusal::BadRequest),
            "{request:?}"
        );
    }
    let unknown = line(serde_json::json!({
        "version": 1, "nonce": NONCE,
        "request": { "kind": "screenshot", "args": { "script": "x" } }
    }));
    assert_eq!(check(&unknown, NONCE), Err(Refusal::BadRequest));
}

#[test]
fn an_output_folder_must_be_absolute_and_plain() {
    let dir = shots_dir();
    assert_eq!(check_out_dir(&dir), Ok(std::path::PathBuf::from(&dir)));
    assert!(check_out_dir("shots").is_err());
    assert!(check_out_dir("").is_err());
    assert!(check_out_dir(&format!("{dir}/../etc")).is_err());
    assert!(check_out_dir(&format!("{dir}\\..\\etc")).is_err());
    assert!(check_out_dir(&format!("{dir}/./x")).is_err());
    assert!(check_out_dir(&"x".repeat(MAX_OUT_LEN + 1)).is_err());
}

#[test]
fn a_screenshot_waits_for_its_delay_and_a_margin() {
    assert_eq!(
        screenshot_budget(&full_page()),
        Duration::from_secs(5) + SCREENSHOT_MARGIN
    );
    assert_eq!(
        screenshot_budget(&ScreenshotArgs::default()),
        SCREENSHOT_MARGIN
    );
}

#[test]
fn answers_and_failures_round_trip_and_failures_are_short_and_plain() {
    let answered = Reply::answered(serde_json::json!({ "shots": [] }));
    assert_eq!(decode_reply(&encode_reply(&answered)), Some(answered));
    let failed = Reply::failed(&format!("no view\u{1b}[2J shows {}", "x".repeat(4096)));
    assert_eq!(failed.error, Some(Refusal::Failed));
    let message = failed.message.clone().expect("a message");
    assert!(message.len() <= MAX_MESSAGE_LEN);
    assert!(message.starts_with("no view[2J shows"));
    assert!(!message.chars().any(char::is_control));
    assert_eq!(decode_reply(&encode_reply(&failed)), Some(failed));
    let cut = Reply::failed(&"\u{e9}".repeat(MAX_MESSAGE_LEN));
    assert!(cut.message.expect("a message").len() <= MAX_MESSAGE_LEN);
}

#[test]
fn a_flow_run_takes_a_plain_name_views_and_run_id_and_nothing_else() {
    let request = json_line("login", &["mobile", "2"], "20261111-101500-a1b2c3");
    assert_eq!(
        check(&request, NONCE),
        Ok(Request::FlowRun {
            name: "login".to_owned(),
            views: vec!["mobile".to_owned(), "2".to_owned()],
            run_id: "20261111-101500-a1b2c3".to_owned(),
        })
    );
    for bad in [
        json_line("../config", &[], "r1"),
        json_line("login", &[], "../out"),
        json_line("login", &[""], "r1"),
    ] {
        assert_eq!(check(&bad, NONCE), Err(Refusal::BadRequest));
    }
    let many: Vec<String> = (0..=MAX_FLOW_VIEWS).map(|i| i.to_string()).collect();
    let many: Vec<&str> = many.iter().map(String::as_str).collect();
    assert_eq!(
        check(&json_line("login", &many, "r1"), NONCE),
        Err(Refusal::BadRequest)
    );
}

fn json_line(name: &str, views: &[&str], run_id: &str) -> Vec<u8> {
    line(serde_json::json!({
        "version": PROTOCOL_VERSION,
        "nonce": NONCE,
        "request": { "kind": "flowRun", "name": name, "views": views, "runId": run_id },
    }))
}
