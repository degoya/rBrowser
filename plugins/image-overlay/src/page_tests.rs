use super::*;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::json;

use crate::image::{base64, tests::png};
use crate::store::{STATE_FILE, STATE_PATH};

fn view(id: u32, breakpoint: &str, width: u32, dpr: f32) -> ViewInfo {
    ViewInfo {
        id,
        label: breakpoint.to_owned(),
        breakpoint: breakpoint.to_owned(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr,
        url: "https://a.test/".to_owned(),
        title: String::new(),
    }
}

/// A PNG of `len` bytes: a real header, then filler the sniffing never reads.
fn big_png(len: usize) -> Vec<u8> {
    let mut bytes = png(393, 852);
    bytes.extend((0..len - bytes.len()).map(|index| (index % 251) as u8));
    bytes
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .with_file(".rbrowser/designs/mobile.png", &big_png(100_000))
        .with_file(".rbrowser/designs/desktop.png", &png(1440, 900));
    host.views = vec![view(1, "mobile", 375, 3.0), view(2, "tablet", 768, 2.0)];
    host.settings.insert(
        "designs".into(),
        r#"{"mobile": "designs/mobile.png", "desktop": "designs/desktop.png"}"#.into(),
    );
    host
}

fn ask(host: &mut MemoryHost, view: u32, message: Value) -> Result<Value, Failure> {
    let args = json!({ "view": view, "message": message }).to_string();
    handle(host, &args).map(|answer| serde_json::from_str(&answer).expect("json"))
}

#[test]
fn init_answers_the_state_and_the_views_design() {
    let mut host = host();
    let answer = ask(&mut host, 1, json!({ "op": "init" })).expect("answer");
    assert_eq!(answer["state"]["enabled"], json!(false));
    assert_eq!(answer["problem"], Value::Null);
    let image = &answer["image"];
    assert_eq!(image["path"], json!("designs/mobile.png"));
    assert_eq!(image["mime"], json!("image/png"));
    assert_eq!(image["width"], json!(393));
    assert_eq!(image["height"], json!(852));
    assert_eq!(image["scale"], json!(1));
    assert_eq!(image["bytes"], json!(100_000));
    assert_eq!(image["chunks"], json!(3));
    assert_eq!(answer["chunkBytes"], json!(CHUNK_BYTES));
    // The tablet has no entry and takes the nearest.
    let tablet = ask(&mut host, 2, json!({ "op": "init" })).expect("answer");
    assert_eq!(tablet["image"]["path"], json!("designs/mobile.png"));
}

#[test]
fn a_view_the_host_does_not_list_uses_the_pages_width() {
    let mut host = host();
    let answer = ask(
        &mut host,
        9,
        json!({ "op": "init", "width": 1400, "dpr": 1 }),
    )
    .expect("answer");
    assert_eq!(answer["image"]["path"], json!("designs/desktop.png"));
    let bad = ask(&mut host, 9, json!({ "op": "init", "width": -5 })).expect_err("bad width");
    assert_eq!(
        bad.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
}

#[test]
fn a_refused_mapping_is_answered_as_the_views_problem() {
    let mut host = host();
    host.settings.insert(
        "designs".into(),
        r#"{"mobile": "../../etc/passwd.png"}"#.into(),
    );
    let answer = ask(&mut host, 1, json!({ "op": "init" })).expect("answer");
    assert_eq!(answer["image"], Value::Null);
    assert_eq!(
        answer["problem"],
        json!({ "code": "plugin.image-overlay.errors.path_outside",
                "params": { "path": "../../etc/passwd.png" } })
    );
}

#[test]
fn the_chunks_carry_the_file_and_fit_a_message() {
    let mut host = host();
    let init = ask(&mut host, 1, json!({ "op": "init" })).expect("init");
    let hash = init["image"]["hash"].clone();
    let file = big_png(100_000);
    let mut joined = Vec::new();
    for index in 0..3 {
        let request =
            json!({ "op": "chunk", "path": "designs/mobile.png", "hash": hash, "index": index });
        let args = json!({ "view": 1, "message": request }).to_string();
        let text = handle(&mut host, &args).expect("chunk");
        assert!(
            text.len() <= 64 * 1024,
            "chunk {index} is {} bytes",
            text.len()
        );
        let answer: Value = serde_json::from_str(&text).expect("json");
        assert_eq!(answer["index"], json!(index));
        let start = index * CHUNK_BYTES;
        let end = (start + CHUNK_BYTES).min(file.len());
        assert_eq!(answer["data"], json!(base64(&file[start..end])));
        joined.extend_from_slice(&file[start..end]);
    }
    assert_eq!(joined, file);
}

#[test]
fn a_full_chunk_of_the_largest_kind_stays_below_the_limit() {
    let mut host = host().with_file(".rbrowser/designs/huge.png", &big_png(CHUNK_BYTES * 2));
    let hash = crate::image::fingerprint(&big_png(CHUNK_BYTES * 2));
    let request = json!({ "op": "chunk", "path": "designs/huge.png", "hash": hash, "index": 0 });
    let text = handle(
        &mut host,
        &json!({ "view": 1, "message": request }).to_string(),
    )
    .expect("chunk");
    assert!(text.len() < 65_536, "{} bytes", text.len());
}

#[test]
fn a_chunk_outside_the_designs_or_past_the_end_or_of_a_changed_file_is_refused() {
    let mut host = host().with_file(".rbrowser/out/image-overlay/state.json", b"{}");
    let hash = crate::image::fingerprint(&big_png(100_000));
    let refused = |host: &mut MemoryHost, message: Value| {
        ask(host, 1, message)
            .expect_err("refused")
            .code
            .unwrap_or_default()
    };
    assert_eq!(
        refused(
            &mut host,
            json!({ "op": "chunk", "path": "out/image-overlay/state.json", "hash": hash, "index": 0 })
        ),
        "plugin.image-overlay.errors.path_outside"
    );
    assert_eq!(
        refused(
            &mut host,
            json!({ "op": "chunk", "path": "designs/mobile.png", "hash": hash, "index": 3 })
        ),
        "plugin.image-overlay.errors.bad_args"
    );
    assert_eq!(
        refused(
            &mut host,
            json!({ "op": "chunk", "path": "designs/mobile.png", "hash": "00", "index": 0 })
        ),
        "plugin.image-overlay.errors.changed"
    );
}

#[test]
fn the_split_handle_sets_the_position_for_every_view() {
    let mut host = host();
    let answer = ask(&mut host, 1, json!({ "op": "split", "position": 30 })).expect("split");
    assert_eq!(
        answer["state"]["split"],
        json!({ "on": true, "position": 30.0 })
    );
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], json!("all"));
    assert_eq!(
        posts[0]["message"]["state"]["split"]["position"],
        json!(30.0)
    );
    assert_eq!(posts[0]["message"]["reload"], json!(false));
    assert!(host.out.contains_key(STATE_FILE));
    let refused = ask(&mut host, 1, json!({ "op": "split", "position": 130 })).expect_err("range");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
}

#[test]
fn without_a_project_the_pages_copy_of_the_overrides_is_used() {
    let mut host = MemoryHost::new().grant(Capability::Views);
    let overrides = json!({ "enabled": true, "opacity": 20 });
    let answer = ask(
        &mut host,
        1,
        json!({ "op": "init", "overrides": overrides }),
    )
    .expect("init");
    assert_eq!(answer["state"]["enabled"], json!(true));
    assert_eq!(answer["state"]["opacity"], json!(20));
    assert_eq!(answer["image"], Value::Null);
    assert!(!host.project.contains_key(STATE_PATH));
}

#[test]
fn an_unknown_request_is_refused() {
    let mut host = host();
    let refused = ask(&mut host, 1, json!({ "op": "eval" })).expect_err("unknown");
    assert_eq!(
        refused.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
}
