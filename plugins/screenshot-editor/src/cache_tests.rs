use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use super::*;
use crate::testing::{FolderHost, host, png};
use crate::tools::load;

const SHOT: &str = ".rbrowser/out/screenshots/2026-11-12_10-00-00/full.png";

/// A PNG of some three chunks.
fn big() -> Vec<u8> {
    let image = png(400, 300, |x, y| {
        [(x * 7 + y) as u8, (y * 3) as u8, (x ^ y) as u8, 255]
    });
    assert!(image.len() > 2 * CHUNK_BYTES, "{}", image.len());
    image
}

fn ask(host: &mut FolderHost, args: Value) -> Value {
    serde_json::from_str(&load(host, &args.to_string()).expect("load")).expect("json")
}

/// Every chunk after the first, from `first`'s `next`, with its `hash`.
fn rest(host: &mut FolderHost, first: &Value) -> Vec<u8> {
    let mut bytes = STANDARD
        .decode(first["data"].as_str().expect("data"))
        .expect("base64");
    let mut next = first["next"].as_u64();
    while let Some(offset) = next {
        let reply = ask(
            host,
            json!({ "image": SHOT, "offset": offset, "hash": first["hash"] }),
        );
        assert_eq!(reply["hash"], first["hash"]);
        assert_eq!(reply["bytes"], first["bytes"]);
        bytes.extend(
            STANDARD
                .decode(reply["data"].as_str().expect("data"))
                .expect("base64"),
        );
        next = reply["next"].as_u64();
    }
    bytes
}

#[test]
fn the_first_chunk_cuts_the_image_once_and_the_rest_come_without_it() {
    // Charge 9 review M3: a chunk no longer reads and fingerprints the whole file.
    let image = big();
    let mut host = host().file(SHOT, &image);
    let first = ask(&mut host, json!({ "image": SHOT }));
    let hash = first["hash"].as_str().expect("hash").to_owned();
    let chunks = image.len().div_ceil(CHUNK_BYTES);
    let cut: Vec<&String> = host
        .0
        .out
        .keys()
        .filter(|path| path.starts_with(&format!("{CHUNKS_DIR}/{hash}/")))
        .collect();
    assert_eq!(cut.len(), chunks + 1, "the chunks and the length: {cut:?}");
    // The image is gone: only the cut version can answer now.
    host.0.project.remove(SHOT);
    assert_eq!(rest(&mut host, &first), image);
    let past = load(
        &mut host,
        &json!({ "image": SHOT, "offset": image.len() + 1, "hash": hash }).to_string(),
    )
    .expect_err("past the end");
    assert_eq!(
        past.code(),
        Some("plugin.screenshot-editor.errors.bad_args")
    );
    // Opening the same version again cuts nothing new.
    host.0.project.insert(SHOT.to_owned(), image);
    host.0.out.clear();
    ask(&mut host, json!({ "image": SHOT }));
    assert!(host.0.out.is_empty(), "{:?}", host.0.out.keys());
}

#[test]
fn a_damaged_chunk_an_unknown_hash_or_none_reads_the_image() {
    let image = big();
    let mut host = host().file(SHOT, &image);
    let first = ask(&mut host, json!({ "image": SHOT }));
    let hash = first["hash"].as_str().expect("hash").to_owned();
    host.0.project.insert(
        format!("{OWN_ROOT}{CHUNKS_DIR}/{hash}/1.b64"),
        b"short".to_vec(),
    );
    assert_eq!(
        rest(&mut host, &first),
        image,
        "the damaged chunk is read again"
    );
    for other in [json!("0123456789abcdef"), json!("../../x"), Value::Null] {
        let reply = ask(
            &mut host,
            json!({ "image": SHOT, "offset": CHUNK_BYTES, "hash": other }),
        );
        let data = STANDARD
            .decode(reply["data"].as_str().expect("data"))
            .expect("base64");
        assert_eq!(data, image[CHUNK_BYTES..2 * CHUNK_BYTES], "{other}");
    }
}

#[test]
fn a_small_image_is_not_cut_and_no_more_versions_than_the_cap() {
    let mut small = host().file(SHOT, &png(2, 2, |_, _| [0, 0, 0, 255]));
    ask(&mut small, json!({ "image": SHOT }));
    assert!(small.0.out.is_empty(), "one chunk is never cut");
    let image = big();
    let mut host = host().file(SHOT, &image);
    for version in 0..MAX_VERSIONS {
        host.0.project.insert(
            format!("{OWN_ROOT}{CHUNKS_DIR}/{version:016x}/bytes"),
            b"1".to_vec(),
        );
    }
    ask(&mut host, json!({ "image": SHOT }));
    assert!(host.0.out.is_empty(), "the cap holds");
}

#[test]
fn only_fingerprints_name_a_folder() {
    assert!(hash_ok("0123456789abcdef"));
    for bad in [
        "0123456789ABCDEF",
        "0123456789abcde",
        "../0123456789abc",
        "",
    ] {
        assert!(!hash_ok(bad), "{bad}");
    }
}
