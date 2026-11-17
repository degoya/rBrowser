use serde_json::json;

use super::*;
use crate::fake::FakeHost;

const URL: &str = "http://localhost:8080/fonts/index.html";
const FONT: &str = "http://localhost:8080/fonts/inter.woff2";

fn host(answer: Value) -> FakeHost {
    let mut host = FakeHost::default();
    host.view(1, "Desktop", 1280, URL);
    host.pages.insert(1, answer.to_string());
    host
}

#[test]
fn reads_a_part_of_the_file_in_the_view() {
    let mut host = host(json!({ "total": 5, "offset": 0, "data": "AAECAwQ=" }));
    let answer = bytes(&mut host, &json!({ "view": 1, "src": FONT })).expect("bytes");
    assert_eq!(
        answer,
        json!({ "src": FONT, "offset": 0, "total": 5, "data": "AAECAwQ=", "done": true })
    );
    let (view, script) = &host.evaluated[0];
    assert_eq!(*view, 1);
    // The request is JSON inside the expression: data, never code.
    assert!(script.contains(&format!("\"src\":\"{FONT}\"")));
    assert!(script.contains(&format!("\"length\":{CHUNK_BYTES}")));
    assert!(script.contains(&format!("\"max\":{}", DEFAULT_MAX_KB * 1024)));
}

#[test]
fn says_whether_more_parts_follow() {
    let mut host = host(json!({ "total": 100_000, "offset": 40_960, "data": "QUJD" }));
    let answer = bytes(
        &mut host,
        &json!({ "view": 1, "src": FONT, "offset": 40_960 }),
    )
    .expect("bytes");
    assert_eq!(answer["done"], false);
    assert!(host.evaluated[0].1.contains("\"offset\":40960"));
}

#[test]
fn passes_on_why_there_is_no_preview() {
    for reason in ["too_large", "not_listed", "unreadable"] {
        let mut host = host(json!({ "error": reason, "total": 9_000_000 }));
        let answer = bytes(&mut host, &json!({ "view": 1, "src": FONT })).expect("bytes");
        assert_eq!(answer["error"], reason);
        assert_eq!(answer["src"], FONT);
        assert!(answer.get("data").is_none());
    }
}

#[test]
fn takes_the_largest_file_from_the_settings() {
    let mut host = host(json!({ "error": "too_large" }));
    host.settings
        .insert("previewMaxKb".to_owned(), "64".to_owned());
    let answer = bytes(&mut host, &json!({ "view": 1, "src": FONT })).expect("bytes");
    assert_eq!(answer["maxKb"], 64);
    assert!(host.evaluated[0].1.contains("\"max\":65536"));
    // Out of the schema's bounds: held to them.
    host.settings
        .insert("previewMaxKb".to_owned(), "999999".to_owned());
    assert_eq!(max_kb(&mut host), MAX_KB);
}

#[test]
fn refuses_what_is_no_font_file_address() {
    let mut host = host(json!({}));
    for args in [
        json!({ "view": 1 }),
        json!({ "view": 1, "src": "file:///C:/Windows/Fonts/arial.ttf" }),
        json!({ "view": 1, "src": "data:font/woff2;base64,AAAA" }),
        json!({ "view": 1, "src": "javascript:alert(1)" }),
        json!({ "view": 1, "src": format!("https://x.test/{}", "a".repeat(MAX_SRC_CHARS)) }),
        json!({ "view": 1, "src": "https://x.test/a\u{0}b.woff2" }),
        json!({ "src": FONT }),
        json!({ "view": 1, "src": FONT, "offset": -1 }),
        json!({ "view": 1, "src": FONT, "offset": 5_000_000_000_u64 }),
    ] {
        let failure = bytes(&mut host, &args).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::INVALID), "{args}");
    }
    assert!(host.evaluated.is_empty());
}

#[test]
fn refuses_a_malformed_answer() {
    for answer in [
        json!({ "total": 5, "offset": 0, "data": "<script>" }),
        json!({ "total": 5, "offset": 7, "data": "AAAA" }),
        json!({ "total": 5 }),
        json!({ "error": "something else" }),
        json!({ "total": 5, "offset": 0, "data": "A".repeat(CHUNK_BYTES * 2) }),
    ] {
        let mut host = host(answer.clone());
        let failure = bytes(&mut host, &json!({ "view": 1, "src": FONT })).expect_err("refused");
        assert_eq!(failure.code(), Some(codes::HOST_FAILED), "{answer}");
    }
}

#[test]
fn a_part_fits_the_panel_message() {
    // Base64 of a full part, in the answer with its address, stays under the panels window's
    // 64 KiB per message (RB-080-63).
    let data = "A".repeat(CHUNK_BYTES.div_ceil(3) * 4);
    let answer = json!({ "src": "x".repeat(MAX_SRC_CHARS), "offset": 0, "total": 1, "data": data });
    let message = json!({ "type": "result", "id": 1, "ok": true, "value": answer });
    assert!(message.to_string().len() < 64 * 1024);
}

#[test]
fn counts_the_bytes_of_a_padded_part() {
    // 40 961 bytes in all: the first part, 40 960 bytes as base64 with two `=`, is not the last.
    let data = "A".repeat(CHUNK_BYTES.div_ceil(3) * 4 - 2) + "==";
    let mut host = host(json!({ "total": CHUNK_BYTES + 1, "offset": 0, "data": data }));
    let answer = bytes(&mut host, &json!({ "view": 1, "src": FONT })).expect("bytes");
    assert_eq!(answer["done"], false);
}
