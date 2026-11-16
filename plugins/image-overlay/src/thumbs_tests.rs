use super::*;
use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;
use serde_json::json;

use crate::image::{base64, tests::png};

const HASH: &str = "0123456789abcdef";

fn host() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite)
}

/// What the next call reads: the files the last one wrote below the out folder.
fn persist(host: &mut MemoryHost) {
    for (path, bytes) in host.out.clone() {
        host.project
            .insert(format!(".rbrowser/out/image-overlay/{path}"), bytes);
    }
}

#[test]
fn a_preview_is_kept_once_and_read_back_by_its_fingerprint() {
    let mut host = host();
    assert_eq!(
        read(&mut host, HASH).expect("read"),
        json!({ "thumb": HASH, "data": null })
    );
    let small = png(96, 128);
    let saved = store(&mut host, HASH, &base64(&small)).expect("store");
    assert_eq!(saved, json!({ "thumb": HASH, "saved": true }));
    assert_eq!(host.out.get(&format!("thumbs/{HASH}.png")), Some(&small));
    persist(&mut host);
    assert_eq!(
        read(&mut host, HASH).expect("read"),
        json!({ "thumb": HASH, "data": base64(&small) })
    );
}

#[test]
fn only_a_small_png_under_a_fingerprint_is_kept() {
    let mut host = host();
    let refused = |host: &mut MemoryHost, hash: &str, data: &str| {
        store(host, hash, data)
            .expect_err("refused")
            .code
            .unwrap_or_default()
    };
    let bad = "plugin.image-overlay.errors.bad_args";
    assert_eq!(refused(&mut host, "../x", &base64(&png(10, 10))), bad);
    assert_eq!(refused(&mut host, HASH, "not base64!"), bad);
    assert_eq!(refused(&mut host, HASH, &base64(b"GIF89a")), bad);
    assert_eq!(
        refused(&mut host, HASH, &base64(&png(MAX_SIDE + 1, 10))),
        bad
    );
    let mut big = png(10, 10);
    big.resize(MAX_BYTES + 1, 0);
    assert_eq!(refused(&mut host, HASH, &base64(&big)), bad);
    assert!(host.out.is_empty());
    assert_eq!(
        read(&mut host, "../x")
            .expect_err("bad hash")
            .code
            .as_deref(),
        Some(bad)
    );
}

#[test]
fn a_file_that_is_no_preview_reads_as_none() {
    let mut host = host().with_file(
        &format!(".rbrowser/out/image-overlay/thumbs/{HASH}.png"),
        b"GIF89a",
    );
    assert_eq!(read(&mut host, HASH).expect("read")["data"], json!(null));
}

#[test]
fn the_previews_are_bounded_in_number() {
    let mut host = host();
    for index in 0..MAX_THUMBS {
        host = host.with_file(
            &format!(".rbrowser/out/image-overlay/thumbs/{index:016x}.png"),
            &png(1, 1),
        );
    }
    let saved = store(&mut host, HASH, &base64(&png(10, 10))).expect("store");
    assert_eq!(saved["saved"], json!(false));
    assert!(host.out.is_empty());
    // One already kept is replaced.
    let kept = format!("{:016x}", 3);
    let again = store(&mut host, &kept, &base64(&png(10, 10))).expect("store");
    assert_eq!(again["saved"], json!(true));
}
