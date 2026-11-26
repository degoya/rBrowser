//! Large images through the tools (RB-099-24): an original over one read, a long page shown as
//! its preview, an image over the capture's limits.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use rb_plugin_api::MAX_IO_BYTES;
use serde_json::{Value, json};

use super::*;
use crate::testing::{FolderHost, header_only, host, png};

const SHOT: &str = ".rbrowser/out/screenshots/2026-11-26_09-00-00/long.png";
const EDITED: &str = "screenshots/2026-11-26_09-00-00/long.edited.png";

fn answer(text: &str) -> Value {
    serde_json::from_str(text).expect("answer")
}

/// Noise: a PNG that does not compress, over one read of 8 MiB.
fn over_one_read() -> Vec<u8> {
    // RGB noise of 9 MB: over 8 MiB however the constant alpha compresses.
    let bytes = png(2000, 1500, crate::testing::noise);
    assert!(bytes.len() > MAX_IO_BYTES, "{}", bytes.len());
    bytes
}

/// The panel's loop: every chunk of the first answer's version.
fn load_all(host: &mut FolderHost, image: &str) -> (Value, Vec<u8>) {
    let first = answer(&load(host, &json!({ "image": image }).to_string()).expect("first"));
    let hash = first["hash"].as_str().expect("hash").to_owned();
    let mut bytes = STANDARD
        .decode(first["data"].as_str().expect("data"))
        .expect("base64");
    let mut next = first["next"].as_u64();
    while let Some(offset) = next {
        let args = json!({ "image": image, "offset": offset, "hash": hash });
        let reply = answer(&load(host, &args.to_string()).expect("chunk"));
        assert!(reply.to_string().len() <= ANSWER_BYTES);
        bytes.extend(
            STANDARD
                .decode(reply["data"].as_str().expect("data"))
                .expect("base64"),
        );
        next = reply["next"].as_u64();
    }
    (first, bytes)
}

#[test]
fn an_image_over_one_read_is_annotated_redacted_and_saved_at_its_size() {
    let original = over_one_read();
    let mut host = host().file(SHOT, &original);
    let annotations = json!([
        { "type": "arrow", "x1": 100, "y1": 100, "x2": 900, "y2": 1300, "color": "#e53935", "width": 6 },
        { "type": "text", "x": 40, "y": 700, "text": "Here", "color": "#000000", "size": 48 },
        { "type": "redact", "x": 200, "y": 650, "w": 600, "h": 300, "mode": "blur", "block": 16 }
    ]);
    let args = json!({ "image": SHOT, "annotations": annotations });
    let reply = answer(&annotate(&mut host, &args.to_string()).expect("annotate"));
    assert_eq!(
        (reply["width"].as_u64(), reply["height"].as_u64()),
        (Some(2000), Some(1500))
    );
    let written = &host.0.out[EDITED];
    assert_eq!(reply["bytes"].as_u64(), Some(written.len() as u64));
    // The same pixels the whole image renders, within a step of anti-aliasing at band edges.
    let mut list: Vec<crate::model::Annotation> =
        serde_json::from_value(annotations).expect("annotations");
    crate::model::validate(&mut list).expect("valid");
    let whole = crate::render::render(codec::decode(&original).expect("decode"), &list)
        .expect("render")
        .take_demultiplied();
    let saved = image::load_from_memory(written).expect("png").into_rgba8();
    assert_eq!(saved.dimensions(), (2000, 1500));
    crate::band::tests::assert_close(saved.as_raw(), &whole, "saved");
    // Read in ranges, never whole.
    assert!(host.0.ranges.len() > 2);
    assert!(host.0.staged.is_none(), "committed");
}

#[test]
fn an_image_over_one_read_loads_in_chunks_that_add_up_to_it() {
    let original = over_one_read();
    let mut host = host().file(SHOT, &original);
    let (first, bytes) = load_all(&mut host, SHOT);
    assert_eq!(first["scale"], 1);
    assert_eq!(first["bytes"].as_u64(), Some(original.len() as u64));
    assert_eq!(bytes, original);
}

#[test]
fn a_page_longer_than_a_canvas_loads_as_its_preview() {
    // 20000 rows: over 16384, so the panel gets every second row and column, averaged.
    let long = png(6, 20_000, |x, y| [(x * 40) as u8, (y % 256) as u8, 9, 255]);
    let mut host = host().file(SHOT, &long);
    let (first, bytes) = load_all(&mut host, SHOT);
    assert_eq!(first["scale"], 2);
    assert_eq!(
        (first["width"].as_u64(), first["height"].as_u64()),
        (Some(6), Some(20_000))
    );
    let preview = image::load_from_memory(&bytes).expect("png").into_rgba8();
    assert_eq!(preview.dimensions(), (3, 10_000));
    // Columns 0-1, rows 0-1: red (0 + 40) / 2, green (0 + 1) / 2 rounded.
    assert_eq!(preview.get_pixel(0, 0).0, [20, 1, 9, 255]);
    // The file itself is saved at its own size.
    let args = json!({ "image": SHOT, "annotations": [{ "type": "marker", "x": 3, "y": 19_990, "number": 1, "color": "#000000", "size": 8 }] });
    let reply = answer(&annotate(&mut host, &args.to_string()).expect("annotate"));
    assert_eq!(
        (reply["width"].as_u64(), reply["height"].as_u64()),
        (Some(6), Some(20_000))
    );
}

#[test]
fn an_image_over_the_capture_limits_names_them_on_load_and_save() {
    for (width, height) in [(1, 65_536), (16_385, 16_384)] {
        let mut host = host().file(SHOT, &header_only(width, height));
        let refused =
            load(&mut host, &json!({ "image": SHOT }).to_string()).expect_err("over the limit");
        assert_eq!(
            refused.code(),
            Some("plugin.screenshot-editor.errors.image_too_large")
        );
        assert_eq!(refused.param("max_side"), Some("65535"));
        assert_eq!(refused.param("max_pixels"), Some("268435456"));
        let args = json!({ "image": SHOT, "annotations": [] });
        let refused = annotate(&mut host, &args.to_string()).expect_err("over the limit");
        assert_eq!(
            refused.code(),
            Some("plugin.screenshot-editor.errors.image_too_large")
        );
        assert!(host.0.out.is_empty(), "nothing written");
    }
}

#[test]
fn a_failed_render_leaves_no_edited_file() {
    // Half of a PNG: the rows run out while the edited file is being written.
    let mut broken = png(64, 64, |x, y| {
        [((x * 7) ^ (y * 13)) as u8, (x * y) as u8, y as u8, 255]
    });
    broken.truncate(broken.len() / 2);
    let mut host = host().file(SHOT, &broken);
    let args = json!({ "image": SHOT, "annotations": [] });
    assert!(annotate(&mut host, &args.to_string()).is_err());
    host.0.end_call();
    assert!(host.0.out.is_empty(), "{:?}", host.0.out.keys());
}

/// The fingerprint `load` names a version by.
fn fingerprint(bytes: &[u8]) -> String {
    format!("{:016x}", cache::fnv(cache::FNV_START, bytes))
}

fn cut(host: &FolderHost, hash: &str) -> bool {
    host.0
        .out
        .contains_key(&format!("{}/{hash}/bytes", cache::CHUNKS_DIR))
}

#[test]
fn a_file_replaced_while_it_is_read_is_answered_and_cut_as_one_version() {
    // Review of RB-099-24: the fingerprint, the answer's data and the cut all of one version.
    let old = png(200, 200, crate::testing::noise);
    let new = png(200, 200, |x, y| crate::testing::noise(y, x));
    assert!(old.len() > CHUNK_BYTES);
    // Replaced after the read that fingerprints it, before the cut reads it again.
    let mut host = host().file(SHOT, &old).swap(1, SHOT, &new);
    let first = answer(&load(&mut host, &json!({ "image": SHOT }).to_string()).expect("first"));
    let hash = fingerprint(&old);
    assert_eq!(first["hash"], hash.as_str());
    let data = STANDARD
        .decode(first["data"].as_str().expect("data"))
        .expect("base64");
    assert_eq!(data, old[..CHUNK_BYTES]);
    assert!(
        !cut(&host, &hash),
        "the new bytes are not cut under the old name"
    );
    // A later chunk names the old version: the file it comes from says it changed.
    let args = json!({ "image": SHOT, "offset": CHUNK_BYTES, "hash": hash });
    let later = answer(&load(&mut host, &args.to_string()).expect("chunk"));
    assert_eq!(later["hash"], fingerprint(&new).as_str());
}

#[test]
fn a_long_page_replaced_while_its_preview_is_made_says_so() {
    let old = png(6, 20_000, |x, y| [(x * 40) as u8, (y % 256) as u8, 9, 255]);
    let new = png(6, 20_000, |x, y| [(x * 40) as u8, 9, (y % 256) as u8, 255]);
    let mut host = host().file(SHOT, &old).swap(1, SHOT, &new);
    let refused = load(&mut host, &json!({ "image": SHOT }).to_string()).expect_err("changed");
    assert_eq!(
        refused.code(),
        Some("plugin.screenshot-editor.panel.changed")
    );
    assert!(!cut(&host, &fingerprint(&old)));
    assert!(!cut(&host, &fingerprint(&new)));
}

#[test]
fn a_preview_cut_once_is_never_made_again() {
    let long = png(6, 20_000, |x, y| [(x * 40) as u8, (y % 256) as u8, 9, 255]);
    let mut host = host().file(SHOT, &long);
    let args = json!({ "image": SHOT }).to_string();
    let first = answer(&load(&mut host, &args).expect("first"));
    assert!(
        cut(&host, &fingerprint(&long)),
        "cut, though it may fit one chunk"
    );
    // Each decode reads the file in ranges; the fingerprint alone is one read of this file.
    let before = host.0.ranges.len();
    let again = answer(&load(&mut host, &args).expect("again"));
    assert_eq!(
        host.0.ranges.len() - before,
        1,
        "fingerprinted, not decoded"
    );
    assert_eq!(again["data"], first["data"]);
    assert_eq!(again["scale"], 2);
    let before = host.0.ranges.len();
    let hash = first["hash"].as_str().expect("hash");
    let refetch = json!({ "image": SHOT, "offset": 0, "hash": hash }).to_string();
    answer(&load(&mut host, &refetch).expect("refetch"));
    assert_eq!(host.0.ranges.len(), before, "from the cache alone");
}
