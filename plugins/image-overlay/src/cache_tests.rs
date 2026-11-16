use super::*;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, Failure, ViewInfo};
use serde_json::{Value, json};

use crate::image::{base64, fingerprint, tests::png};
use crate::page::handle;

const DESIGN: &str = ".rbrowser/designs/mobile.png";

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
        .with_file(DESIGN, &big_png(100_000));
    host.views = vec![ViewInfo {
        id: 1,
        label: "mobile".to_owned(),
        breakpoint: "mobile".to_owned(),
        engine: "chromium".to_owned(),
        width: 375,
        height: 800,
        dpr: 3.0,
        url: "https://a.test/".to_owned(),
        title: String::new(),
    }];
    host.settings.insert(
        "designs".into(),
        r#"{"mobile": "designs/mobile.png"}"#.into(),
    );
    host
}

/// What `out.write` wrote, readable through `project.read` as on the real host.
fn publish(host: &mut MemoryHost) {
    for (path, bytes) in std::mem::take(&mut host.out) {
        host.project
            .insert(format!("{OUT_PROJECT_DIR}/{path}"), bytes);
    }
}

fn ask(host: &mut MemoryHost, message: Value) -> Result<Value, Failure> {
    let args = json!({ "view": 1, "message": message }).to_string();
    handle(host, &args).map(|answer| serde_json::from_str(&answer).expect("json"))
}

fn chunk(host: &mut MemoryHost, hash: &Value, index: usize) -> Result<Value, Failure> {
    ask(
        host,
        json!({ "op": "chunk", "path": "designs/mobile.png", "hash": hash, "index": index }),
    )
}

#[test]
fn init_cuts_the_design_once_and_the_chunks_come_without_the_design() {
    // Charge 8 review M3: a chunk no longer reads and fingerprints the whole file.
    let mut host = host();
    let init = ask(&mut host, json!({ "op": "init" })).expect("init");
    let hash = init["image"]["hash"].clone();
    assert_eq!(host.out.len(), 4, "three chunks and the length");
    publish(&mut host);
    host.project.remove(DESIGN);
    let file = big_png(100_000);
    for index in 0..3 {
        let answer = chunk(&mut host, &hash, index).expect("cached chunk");
        let start = index * CHUNK_BYTES;
        let end = (start + CHUNK_BYTES).min(file.len());
        assert_eq!(answer["data"], json!(base64(&file[start..end])), "{index}");
    }
    let past = chunk(&mut host, &hash, 3).expect_err("past the end");
    assert_eq!(
        past.code.as_deref(),
        Some("plugin.image-overlay.errors.bad_args")
    );
    host.project.insert(DESIGN.to_owned(), file);
    ask(&mut host, json!({ "op": "init" })).expect("init again");
    assert!(host.out.is_empty(), "a cut version is not written again");
}

#[test]
fn a_damaged_or_missing_chunk_falls_back_to_the_design() {
    let mut host = host();
    let init = ask(&mut host, json!({ "op": "init" })).expect("init");
    let hash = init["image"]["hash"].clone();
    publish(&mut host);
    let key = |index: usize| {
        format!(
            "{OUT_PROJECT_DIR}/{CHUNKS_DIR}/{}/{index}.b64",
            hash.as_str().expect("hash")
        )
    };
    host.project.insert(key(1), b"QUJD".to_vec());
    host.project.remove(&key(2));
    let file = big_png(100_000);
    let answer = chunk(&mut host, &hash, 1).expect("from the design");
    assert_eq!(
        answer["data"],
        json!(base64(&file[CHUNK_BYTES..2 * CHUNK_BYTES]))
    );
    let answer = chunk(&mut host, &hash, 2).expect("from the design");
    assert_eq!(answer["data"], json!(base64(&file[2 * CHUNK_BYTES..])));
}

#[test]
fn a_hash_that_names_no_cut_version_is_read_from_the_design() {
    let mut host = host();
    publish(&mut host);
    for hash in ["../../state", "00", "ABCDEF0123456789"] {
        assert!(!hash_ok(hash), "{hash}");
        let answer = chunk(&mut host, &json!(hash), 0).expect_err("not this file");
        assert_eq!(
            answer.code.as_deref(),
            Some("plugin.image-overlay.errors.changed")
        );
    }
    assert!(hash_ok(&fingerprint(b"x")));
}

#[test]
fn no_more_versions_are_cut_than_the_cap() {
    let mut host = host();
    for version in 0..MAX_VERSIONS {
        host.project.insert(
            format!("{OUT_PROJECT_DIR}/{CHUNKS_DIR}/{version:016x}/bytes"),
            b"1".to_vec(),
        );
    }
    let init = ask(&mut host, json!({ "op": "init" })).expect("init");
    assert!(host.out.is_empty());
    assert_eq!(host.logs.len(), 1);
    let hash = init["image"]["hash"].clone();
    chunk(&mut host, &hash, 0).expect("from the design");
}
