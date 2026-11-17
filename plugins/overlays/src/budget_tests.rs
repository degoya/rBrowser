use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};

use super::*;
use crate::Overlays;

/// The most an answer may be with its reserve kept: under the host's limit with room to spare.
const FITS: usize = MESSAGE_BYTES - RESERVE_BYTES / 2;

fn host(views: u32) -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::Evaluate)
        .grant(Capability::Findings)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = (1..=views)
        .map(|id| ViewInfo {
            id,
            label: format!("view {id}"),
            breakpoint: format!("bp{id}"),
            engine: "chromium".to_owned(),
            url: "http://localhost/big.html".to_owned(),
            ..ViewInfo::default()
        })
        .collect();
    host
}

/// A text of `chars` characters, multi-byte ones when `wide` (each three bytes in UTF-8).
fn text(seed: usize, chars: usize, wide: bool) -> String {
    let unit = if wide { '\u{4e2d}' } else { 'x' };
    let mut text = format!("div.c{seed} > ");
    while text.chars().count() < chars {
        text.push(unit);
    }
    text.chars().take(chars).collect()
}

/// A big page's scan: `count` stacking contexts, 200 listed, every text at its longest.
fn scan_answer(count: usize, wide: bool) -> String {
    let items: Vec<Value> = (0..200)
        .map(|n| {
            json!({
                "selector": text(n, 300, wide),
                "path": text(n + 1000, 300, wide),
                "label": "z:auto position: fixed +3",
                "rect": { "x": 1_234.567_891, "y": 98_765.432_1, "width": 1280.5, "height": 640.25 },
            })
        })
        .collect();
    json!({ "count": count, "items": items }).to_string()
}

/// A big page's tab order: 250 stops, every text at its longest.
fn tab_answer(wide: bool) -> String {
    let items: Vec<Value> = (0..250)
        .map(|n| {
            json!({
                "index": n + 1,
                "selector": text(n, 300, wide),
                "path": text(n + 1000, 300, wide),
                "tabindex": 0,
                "rect": { "x": 1.5, "y": 2.5, "width": 100.25, "height": 40.75 },
                "warnings": ["invisible-offscreen", "reverse-jump"],
            })
        })
        .collect();
    json!({ "count": 1234, "items": items }).to_string()
}

/// The answer as the panel gets it: the posts taken out by the host.
fn delivered(answer: &str) -> (Value, usize) {
    let mut value: Value = serde_json::from_str(answer).expect("JSON");
    if let Some(fields) = value.as_object_mut() {
        fields.remove("$post");
    }
    let size = value.to_string().len();
    (value, size)
}

#[test]
fn stacking_on_a_big_page_answers_within_the_message_limit_with_the_whole_count() {
    for views in [1, 3, 8, 16] {
        for wide in [false, true] {
            let mut host = host(views);
            host.evaluate_answer = scan_answer(1234, wide);
            let answer = Overlays
                .call_tool(
                    &mut host,
                    "set",
                    &json!({ "overlay": "stacking" }).to_string(),
                )
                .expect("on");
            let (answer, size) = delivered(&answer);
            assert!(size <= FITS, "{views} views, wide {wide}: {size} bytes");
            let hits = answer["hits"].as_array().expect("hits");
            assert_eq!(hits.len(), views as usize);
            for hit in hits {
                assert_eq!(hit["count"], json!(1234));
                let items = hit["items"].as_array().expect("items").len();
                assert!(items >= 1, "{views} views: every view lists a hit");
                assert_eq!(hit["shown"], json!(items));
            }
        }
    }
}

#[test]
fn every_checking_overlay_answers_within_the_message_limit() {
    for overlay in ["scrollbars", "text-spacing", "target-size", "small-text"] {
        let mut host = host(8);
        host.evaluate_answer = scan_answer(500, true);
        let answer = Overlays
            .call_tool(&mut host, "set", &json!({ "overlay": overlay }).to_string())
            .expect("on");
        let (_, size) = delivered(&answer);
        assert!(size <= FITS, "{overlay}: {size} bytes");
    }
}

#[test]
fn a_small_page_lists_every_hit_it_answered_before() {
    let mut host = host(2);
    host.evaluate_answer = json!({
        "count": 3,
        "items": [
            { "selector": "a", "path": "html > body > a", "label": "z:1 opacity", "rect": {} },
            { "selector": "b", "path": "html > body > b", "label": "z:2 opacity", "rect": {} },
            { "selector": "i", "path": "html > body > i", "label": "z:3 opacity", "rect": {} },
        ],
    })
    .to_string();
    let answer = Overlays
        .call_tool(
            &mut host,
            "set",
            &json!({ "overlay": "stacking" }).to_string(),
        )
        .expect("on");
    let (answer, _) = delivered(&answer);
    for hit in answer["hits"].as_array().expect("hits") {
        assert_eq!(hit["count"], json!(3));
        assert_eq!(hit["shown"], json!(3));
        assert_eq!(hit["items"].as_array().map(Vec::len), Some(3));
    }
}

#[test]
fn the_tab_order_on_a_big_page_answers_within_the_message_limit() {
    for views in [1, 4, 16] {
        let mut host = host(views);
        host.evaluate_answer = tab_answer(true);
        let switched = Overlays
            .call_tool(
                &mut host,
                "set",
                &json!({ "overlay": "tab-order" }).to_string(),
            )
            .expect("on");
        let (switched, size) = delivered(&switched);
        assert!(size <= FITS, "set, {views} views: {size} bytes");
        let compared = Overlays
            .call_tool(&mut host, "tab_order", "{}")
            .expect("tab order");
        let (compared, size) = delivered(&compared);
        assert!(size <= FITS, "tab_order, {views} views: {size} bytes");
        for answer in [&switched["tabOrder"], &compared] {
            for entry in answer["views"].as_array().expect("views") {
                assert_eq!(entry["count"], json!(1234));
                let items = entry["items"].as_array().expect("items").len();
                assert!(items >= 1);
                assert_eq!(entry["shown"], json!(items));
            }
        }
    }
}

#[test]
fn the_lists_share_the_budget_in_turns_each_at_most_most() {
    // Each item is `"xxxx"`: six bytes, seven with its comma.
    let long = vec!["xxxx"; 10];
    let short = vec!["xxxx"; 2];
    let empty: Vec<&str> = Vec::new();
    let lists = [long.as_slice(), short.as_slice(), empty.as_slice()];
    assert_eq!(share(&lists, 50, 7 * 12), vec![10, 2, 0]);
    assert_eq!(share(&lists, 50, 7 * 6), vec![4, 2, 0]);
    assert_eq!(share(&lists, 50, 7 * 3), vec![2, 1, 0]);
    assert_eq!(share(&lists, 3, 1000), vec![3, 2, 0]);
    assert_eq!(share(&lists, 50, 6), vec![0, 0, 0]);
}
