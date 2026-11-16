use super::*;
use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use crate::image::{base64, fingerprint, tests::png};

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
        .with_file(".rbrowser/designs/mobile@2x.png", &png(786, 1704))
        .with_file(".rbrowser/designs/desktop-1440.png", &png(1440, 900))
        .with_file(".rbrowser/designs/broken.png", b"not a png")
        .with_file(".rbrowser/designs/notes.txt", b"x")
}

fn call(host: &mut MemoryHost, args: Value) -> Result<Value, Failure> {
    run(host, &args.to_string()).map(|answer| serde_json::from_str(&answer).expect("json"))
}

#[test]
fn the_list_names_each_design_with_its_size_or_why_it_is_unusable() {
    let mut host = host();
    let answer = call(&mut host, json!({})).expect("list");
    let designs = answer["designs"].as_array().expect("a list");
    let names: Vec<&str> = designs
        .iter()
        .map(|design| design["name"].as_str().expect("a name"))
        .collect();
    // Sorted by name; a file that is no design by its name is not listed.
    assert_eq!(names, ["broken.png", "desktop-1440.png", "mobile@2x.png"]);
    assert_eq!(designs[0]["path"], json!("designs/broken.png"));
    assert_eq!(
        designs[0]["problem"]["code"],
        json!("plugin.image-overlay.errors.bad_type")
    );
    assert_eq!(designs[1]["width"], json!(1440));
    assert_eq!(designs[1]["height"], json!(900));
    assert_eq!(designs[2]["width"], json!(786));
    assert_eq!(designs[2]["scale"], json!(2));
    assert_eq!(designs[2]["mime"], json!("image/png"));
    assert_eq!(designs[2]["hash"], json!(fingerprint(&png(786, 1704))));
    assert_eq!(answer["more"], json!(false));
    assert_eq!(answer["chunkBytes"], json!(CHUNK_BYTES));
}

#[test]
fn the_list_says_whether_a_design_can_be_added() {
    let mut host = host();
    // `.rbrowser/flows` is not there: the host's `not_found` means a trusted project.
    assert_eq!(
        call(&mut host, json!({})).expect("list")["trusted"],
        json!(true)
    );
    let mut none = MemoryHost::new();
    let answer = call(&mut none, json!({})).expect("list without a project");
    assert_eq!(answer["trusted"], json!(false));
    assert_eq!(answer["designs"], json!([]));
}

#[test]
fn a_long_folder_is_listed_in_part_and_says_so() {
    let mut host = MemoryHost::new().grant(Capability::ProjectRead);
    for index in 0..MAX_LISTED + 3 {
        host = host.with_file(&format!(".rbrowser/designs/d{index:03}.png"), &png(10, 10));
    }
    let answer = call(&mut host, json!({})).expect("list");
    assert_eq!(answer["designs"].as_array().map(Vec::len), Some(MAX_LISTED));
    assert_eq!(answer["more"], json!(true));
}

#[test]
fn a_chunk_of_a_design_is_answered_for_its_fingerprint() {
    let mut host = host();
    let bytes = png(1440, 900);
    let hash = fingerprint(&bytes);
    let answer = call(
        &mut host,
        json!({ "path": "designs/desktop-1440.png", "hash": hash, "chunk": 0 }),
    )
    .expect("chunk");
    assert_eq!(answer["index"], json!(0));
    assert_eq!(answer["data"], json!(base64(&bytes)));
    // The version is cut once, so the next chunk is a small read.
    assert!(
        host.out
            .keys()
            .any(|path| path.starts_with(&format!("chunks/{hash}/")))
    );
    let past = call(
        &mut host,
        json!({ "path": "designs/desktop-1440.png", "hash": hash, "chunk": 1 }),
    )
    .expect_err("past the end");
    assert_eq!(
        past.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
}

#[test]
fn a_chunk_is_refused_outside_the_designs_or_for_another_version() {
    let mut host = host();
    let outside = call(
        &mut host,
        json!({ "path": "../secret.png", "hash": "0000000000000000", "chunk": 0 }),
    )
    .expect_err("outside");
    assert_eq!(
        outside.code.as_deref(),
        Some("plugin.image-overlay.errors.path_outside")
    );
    let changed = call(
        &mut host,
        json!({ "path": "designs/desktop-1440.png", "hash": "0000000000000000", "chunk": 0 }),
    )
    .expect_err("another version");
    assert_eq!(
        changed.code.as_deref(),
        Some("plugin.image-overlay.errors.changed")
    );
    for args in [
        json!({ "path": "designs/desktop-1440.png", "chunk": 0 }),
        json!({ "hash": "0000000000000000", "chunk": 0 }),
        json!({ "path": "designs/desktop-1440.png", "hash": "0000000000000000" }),
        json!({ "chunk": -1 }),
        json!({ "other": 1 }),
    ] {
        let refused = call(&mut host, args.clone()).expect_err("bad arguments");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugin.image-overlay.errors.bad_args"),
            "{args}"
        );
    }
}

#[test]
fn the_previews_are_asked_for_and_kept_through_the_tool() {
    let mut host = host();
    let hash = fingerprint(&png(1440, 900));
    let none = call(&mut host, json!({ "thumb": hash })).expect("read");
    assert_eq!(none["data"], json!(null));
    let kept = call(
        &mut host,
        json!({ "thumb": hash, "data": base64(&png(96, 60)) }),
    )
    .expect("store");
    assert_eq!(kept["saved"], json!(true));
    for args in [
        json!({ "thumb": hash, "chunk": 0 }),
        json!({ "data": base64(&png(96, 60)) }),
    ] {
        let refused = call(&mut host, args.clone()).expect_err("mixed");
        assert_eq!(
            refused.code.as_deref(),
            Some("plugin.image-overlay.errors.bad_args"),
            "{args}"
        );
    }
}
