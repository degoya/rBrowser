use serde_json::{Value, json};

use super::*;
use crate::testing::{FolderHost, at, chunk_types, every_tool, host, png, with_text};

const SHOT: &str = ".rbrowser/out/screenshots/2026-11-12_10-00-00/mobile.png";

/// Black-and-white strokes: the "secret" a redaction must hide.
fn secret(x: u32, y: u32) -> [u8; 4] {
    if (x + 2 * y) % 3 == 0 || (x * y) % 7 == 1 {
        [0, 0, 0, 255]
    } else {
        [255, 255, 255, 255]
    }
}

fn with_shot() -> FolderHost {
    host().file(
        SHOT,
        &with_text(&png(40, 40, secret), "Comment", "user@example.com"),
    )
}

fn answer(text: &str) -> Value {
    serde_json::from_str(text).expect("answer")
}

#[test]
fn annotate_renders_without_ui_and_answers_the_path() {
    let mut host = with_shot();
    let args = json!({ "image": SHOT, "annotations": every_tool(), "target": "all" });
    let reply = answer(&annotate(&mut host, &args.to_string()).expect("annotate"));
    let edited = "screenshots/2026-11-12_10-00-00/mobile.edited.png";
    assert_eq!(
        reply["path"],
        format!(".rbrowser/out/screenshot-editor/{edited}")
    );
    assert_eq!(
        reply["annotations"],
        ".rbrowser/out/screenshot-editor/screenshots/2026-11-12_10-00-00/mobile.annotations.json"
    );
    assert_eq!(reply["format"], "png");
    assert_eq!(
        (reply["width"].as_u64(), reply["height"].as_u64()),
        (Some(40), Some(40))
    );
    let written = &host.0.out[edited];
    assert_eq!(reply["bytes"].as_u64(), Some(written.len() as u64));
    // The original stays as it was.
    assert!(host.0.project[SHOT].len() > 100);
}

#[test]
fn a_pixelated_region_of_the_export_is_the_mosaic_and_the_png_has_no_text() {
    let mut host = with_shot();
    let region = json!({ "type": "redact", "x": 8, "y": 8, "w": 24, "h": 24, "block": 8 });
    let args = json!({ "image": SHOT, "annotations": [region] });
    annotate(&mut host, &args.to_string()).expect("annotate");
    let exported = &host.0.out["screenshots/2026-11-12_10-00-00/mobile.edited.png"];
    let types = chunk_types(exported);
    assert!(
        !types
            .iter()
            .any(|kind| ["tEXt", "iTXt", "zTXt", "tIME", "eXIf"].contains(&kind.as_str())),
        "{types:?}"
    );
    let pixels = image::load_from_memory(exported).expect("png").into_rgba8();
    let raw = pixels.as_raw();
    for by in 0..3 {
        for bx in 0..3 {
            let (left, top) = (8 + bx * 8, 8 + by * 8);
            let mut sum = 0u32;
            for y in top..top + 8 {
                for x in left..left + 8 {
                    sum += u32::from(secret(x, y)[0]);
                }
            }
            let expected = u8::try_from((sum + 32) / 64).expect("grey");
            for y in top..top + 8 {
                for x in left..left + 8 {
                    assert_eq!(at(raw, 40, x, y), [expected, expected, expected, 255]);
                }
            }
        }
    }
    // The original pattern does not occur in the region: no pure black or white is left there.
    for y in 8..32 {
        for x in 8..32 {
            let [r, ..] = at(raw, 40, x, y);
            assert!(r != 0 && r != 255);
        }
    }
    // Outside it the image is the original.
    assert_eq!(at(raw, 40, 0, 0), secret(0, 0));
}

#[test]
fn the_saved_annotations_load_again() {
    let mut host = with_shot();
    let args = json!({ "image": SHOT, "annotations": every_tool() });
    annotate(&mut host, &args.to_string()).expect("annotate");
    let first = answer(&load(&mut host, &json!({ "image": SHOT }).to_string()).expect("load"));
    let saved: Vec<crate::model::Annotation> =
        serde_json::from_value(first["annotations"].clone()).expect("annotations");
    let mut expected: Vec<crate::model::Annotation> =
        serde_json::from_value(every_tool()).expect("tools");
    crate::model::validate(&mut expected).expect("valid");
    assert_eq!(saved, expected);
    assert_eq!(
        (first["width"].as_u64(), first["height"].as_u64()),
        (Some(40), Some(40))
    );
}

#[test]
fn an_image_loads_in_chunks_that_add_up_to_the_file() {
    let big = noisy(400, 300);
    assert!(big.len() > CHUNK_BYTES, "{}", big.len());
    let mut host = host().file(SHOT, &big);
    let mut bytes = Vec::new();
    let mut offset = Some(0u64);
    let mut hash = None;
    while let Some(at) = offset {
        let reply = answer(
            &load(
                &mut host,
                &json!({ "image": SHOT, "offset": at }).to_string(),
            )
            .expect("chunk"),
        );
        assert!(reply.to_string().len() < 64 * 1024);
        bytes.extend(
            STANDARD
                .decode(reply["data"].as_str().expect("data"))
                .expect("base64"),
        );
        assert!(
            hash.is_none() || hash.as_ref() == reply["hash"].as_str().map(str::to_owned).as_ref()
        );
        hash = reply["hash"].as_str().map(str::to_owned);
        offset = reply["next"].as_u64();
        if at > 0 {
            assert!(reply.get("width").is_none());
        }
    }
    assert_eq!(bytes, big);
    assert_eq!(
        load(
            &mut host,
            &json!({ "image": SHOT, "offset": big.len() + 1 }).to_string()
        )
        .expect_err("past the end")
        .code(),
        Some("plugin.screenshot-editor.errors.bad_args")
    );
}

/// The panel's loop over `load`: every answer within a panel message, the chunks in order.
fn load_all(host: &mut FolderHost) -> (Value, Vec<u8>) {
    let first = answer(&load(host, &json!({ "image": SHOT }).to_string()).expect("first"));
    let hash = first["hash"].as_str().expect("hash").to_owned();
    let mut bytes = Vec::new();
    let mut reply = first.clone();
    loop {
        assert!(
            reply.to_string().len() <= ANSWER_BYTES,
            "an answer of {} bytes",
            reply.to_string().len()
        );
        bytes.extend(
            STANDARD
                .decode(reply["data"].as_str().expect("data"))
                .expect("base64"),
        );
        let Some(next) = reply["next"].as_u64() else {
            break;
        };
        let args = json!({ "image": SHOT, "offset": next, "hash": hash });
        reply = answer(&load(host, &args.to_string()).expect("chunk"));
        assert_eq!(reply["hash"], hash.as_str());
        assert!(reply.get("annotations").is_none());
    }
    (first, bytes)
}

fn noisy(width: u32, height: u32) -> Vec<u8> {
    png(width, height, |x, y| {
        [(x * 7 + y) as u8, (y * 3) as u8, (x ^ y) as u8, 255]
    })
}

// RB-080-63: the owner's `Failed: plugins.error.too_large`. The first answer carried a whole
// chunk (47 KiB, 62.7 KiB as base64) and the saved annotations; past some 1.1 KiB of them the
// answer outgrew a panel message (64 KiB, crates/rbrowser/src/plugins/runtime.rs `bounded`), so
// an image above 47 KiB with saved annotations no longer opened.
#[test]
fn a_large_image_with_saved_annotations_loads_within_the_message_limit() {
    let big = noisy(400, 300);
    assert!(big.len() > CHUNK_BYTES, "{}", big.len());
    let mut host = host().file(SHOT, &big);
    let rects: Vec<Value> = (0..40)
        .map(|i| json!({ "type": "rect", "x": i, "y": i, "w": 20, "h": 10, "color": "#e53935", "width": 4 }))
        .collect();
    let args = json!({ "image": SHOT, "annotations": rects });
    annotate(&mut host, &args.to_string()).expect("annotate");
    let (first, bytes) = load_all(&mut host);
    assert_eq!(bytes, big);
    assert_eq!(first["annotations"].as_array().map(Vec::len), Some(40));
    assert_eq!(first["annotations_too_large"], Value::Null);
}

#[test]
fn a_small_image_and_its_annotations_come_in_one_answer() {
    let mut host = with_shot();
    let args = json!({ "image": SHOT, "annotations": [{ "type": "marker", "x": 1, "y": 1, "number": 1, "color": "#000000", "size": 12 }] });
    annotate(&mut host, &args.to_string()).expect("annotate");
    let first = answer(&load(&mut host, &json!({ "image": SHOT }).to_string()).expect("load"));
    assert_eq!(first["next"], Value::Null);
    assert_eq!(first["annotations"].as_array().map(Vec::len), Some(1));
    assert_eq!(
        STANDARD
            .decode(first["data"].as_str().expect("data"))
            .expect("base64"),
        host.0.project[SHOT]
    );
}

#[test]
fn saved_annotations_beyond_a_message_open_the_image_without_them_and_say_so() {
    let big = noisy(400, 300);
    let mut host = host().file(SHOT, &big);
    // shot_annotate over MCP takes more than a panel message: 900 freehand strokes.
    let strokes: Vec<Value> = (0..900)
        .map(|i| json!({ "type": "freehand", "points": [[i % 400, 1], [i % 400, 299], [1, i % 300]], "color": "#e53935", "width": 4 }))
        .collect();
    let args = json!({ "image": SHOT, "annotations": strokes });
    assert!(args.to_string().len() > ANSWER_BYTES);
    annotate(&mut host, &args.to_string()).expect("annotate");
    let (first, bytes) = load_all(&mut host);
    assert_eq!(bytes, big);
    assert_eq!(first["annotations"], Value::Null);
    assert_eq!(first["annotations_too_large"], true);
}

// Review of RB-080-63, 6: the last fallback (annotations out, chunk in) was never measured; a
// long path could still carry the answer past a message. Then the chunk stays out as well.
#[test]
fn the_first_answer_fits_even_when_its_other_fields_are_long() {
    let mut host = host();
    let mut answer = json!({
        "image": "x".repeat(2000),
        "bytes": 200_000,
        "offset": 0,
        "next": CHUNK_BYTES,
        "hash": "0123456789abcdef",
        "data": "A".repeat(CHUNK_BYTES.div_ceil(3) * 4),
        "width": 400,
        "height": 300,
        "annotations": vec![json!({ "type": "rect" }); 5000],
    });
    fit_first(&mut host, &mut answer);
    assert!(
        answer.to_string().len() <= ANSWER_BYTES,
        "{}",
        answer.to_string().len()
    );
    assert_eq!(answer["data"], "");
    assert_eq!(answer["next"], 0);
    assert_eq!(answer["annotations"], Value::Null);
    assert_eq!(answer["annotations_too_large"], true);
}

// Review of RB-080-63, 7: offset 0 with a hash the cache does not hold (an image of one chunk,
// or one not cut) is a first read: it cuts the image and names its size - and always carries
// the data, which is what the call asks for.
#[test]
fn offset_zero_with_a_hash_the_cache_lacks_reads_like_a_first_call_with_its_data() {
    let big = noisy(400, 300);
    let mut host = host().file(SHOT, &big);
    let args = json!({ "image": SHOT, "offset": 0, "hash": "0123456789abcdef" });
    let reply = answer(&load(&mut host, &args.to_string()).expect("load"));
    assert_eq!(
        (reply["width"].as_u64(), reply["height"].as_u64()),
        (Some(400), Some(300))
    );
    assert_eq!(
        STANDARD
            .decode(reply["data"].as_str().expect("data"))
            .expect("base64"),
        &big[..CHUNK_BYTES]
    );
    // Cut now: the next chunk comes from the cache, not from the image.
    let hash = reply["hash"].as_str().expect("hash");
    assert!(matches!(
        cache::chunk(&mut host, hash, CHUNK_BYTES),
        Cached::Data { .. }
    ));
}

#[test]
fn without_an_image_load_takes_the_last_screenshot() {
    let mut host = with_shot().file(
        ".rbrowser/out/screenshots/2026-11-01_08-00-00/mobile.png",
        &png(2, 2, secret),
    );
    let reply = answer(&load(&mut host, "{}").expect("load"));
    assert_eq!(reply["image"], SHOT);
    assert_eq!(reply["annotations"], Value::Null);
    let refused = load(&mut crate::testing::host(), "").expect_err("none");
    assert_eq!(
        refused.code(),
        Some("plugin.screenshot-editor.errors.no_screenshot")
    );
}

#[test]
fn a_target_outside_the_out_folder_is_refused_before_anything_is_written() {
    for out in [
        "docs/bug.png",
        "/tmp/bug.png",
        ".rbrowser/out/screenshots/bug.png",
        "../bug.png",
    ] {
        let mut host = with_shot();
        let args = json!({ "image": SHOT, "annotations": [], "out": out });
        let refused = annotate(&mut host, &args.to_string()).expect_err(out);
        assert_eq!(
            refused.code(),
            Some("plugin.screenshot-editor.errors.path_outside"),
            "{out}"
        );
        assert!(host.0.out.is_empty());
    }
    for image in [
        ".rbrowser/config.json",
        "src/logo.png",
        ".rbrowser/designs/home.png",
    ] {
        let args = json!({ "image": image, "annotations": [] });
        let refused = annotate(&mut with_shot(), &args.to_string()).expect_err(image);
        assert_eq!(
            refused.code(),
            Some("plugin.screenshot-editor.errors.path_outside")
        );
    }
}

#[test]
fn a_named_jpeg_target_and_the_format_setting_choose_the_encoder() {
    let mut host = with_shot();
    let args = json!({ "image": SHOT, "annotations": [], "out": ".rbrowser/out/screenshot-editor/bugs/login.jpg" });
    let reply = answer(&annotate(&mut host, &args.to_string()).expect("jpeg"));
    assert_eq!(
        reply["path"],
        ".rbrowser/out/screenshot-editor/bugs/login.jpg"
    );
    assert_eq!(&host.0.out["bugs/login.jpg"][..2], &[0xFF, 0xD8]);
    assert!(host.0.out.contains_key("bugs/login.annotations.json"));

    let mut host = with_shot();
    host.0
        .settings
        .insert("format".to_owned(), "\"jpeg\"".to_owned());
    let reply = answer(
        &annotate(
            &mut host,
            &json!({ "image": SHOT, "annotations": [] }).to_string(),
        )
        .expect("setting"),
    );
    assert_eq!(reply["format"], "jpeg");
    assert!(
        host.0
            .out
            .contains_key("screenshots/2026-11-12_10-00-00/mobile.edited.jpg")
    );
}

#[test]
fn bad_arguments_name_what_is_wrong() {
    let mut host = with_shot();
    for args in [
        json!({ "image": SHOT, "annotations": [{ "type": "circle" }] }),
        json!({ "image": SHOT, "annotations": [], "script": "x" }),
        json!({ "annotations": [] }),
    ] {
        let refused = annotate(&mut host, &args.to_string()).expect_err("bad");
        assert_eq!(
            refused.code(),
            Some("plugin.screenshot-editor.errors.bad_args"),
            "{args}"
        );
    }
    let bad = json!({ "image": SHOT, "annotations": [{ "type": "text", "x": 0, "y": 0, "text": "", "color": "#000000", "size": 12 }] });
    assert_eq!(
        annotate(&mut host, &bad.to_string())
            .expect_err("text")
            .code(),
        Some("plugin.screenshot-editor.errors.bad_annotation")
    );
}

#[test]
fn the_list_tool_answers_the_images() {
    let mut host = with_shot();
    let reply = answer(&list(&mut host, r#"{"target":"all"}"#).expect("list"));
    assert_eq!(reply, json!({ "images": [SHOT], "truncated": false }));
}
