use serde_json::json;

use super::*;
use crate::fake::FakeHost;
use crate::image::tests::png;

const PAGE: &str = "https://a.test/app/";

fn manifest(value: &serde_json::Value) -> Manifest {
    let url = Url::parse("https://a.test/app/manifest.webmanifest").expect("url");
    let page = Url::parse(PAGE).expect("url");
    Manifest::parse(&value.to_string(), &url, &page).expect("manifest")
}

fn run(host: &mut FakeHost, manifest: &Manifest) -> (Vec<IconReport>, Vec<String>) {
    let page = Url::parse(PAGE).expect("url");
    let budget = Budget::new(host, 30_000);
    let mut sink = Sink::new(&[], PAGE, Some(1));
    let reports = check(host, &budget, &page, manifest, &mut sink);
    let found = sink
        .findings
        .iter()
        .map(|f| {
            let params: Vec<String> = f.params.iter().map(|(k, v)| format!("{k}={v}")).collect();
            format!(
                "{} {} {}",
                f.rule,
                f.path.as_deref().unwrap_or(""),
                params.join(" ")
            )
        })
        .collect();
    (reports, found)
}

#[test]
fn good_icons_give_no_finding_and_a_full_report() {
    let mut host = FakeHost::default();
    host.serve(
        "https://a.test/app/192.png",
        200,
        "image/png",
        &png(192, 192),
    )
    .serve(
        "https://a.test/app/512.png",
        200,
        "image/png",
        &png(512, 512),
    );
    let manifest = manifest(&json!({ "icons": [
        { "src": "192.png", "sizes": "192x192", "type": "image/png" },
        { "src": "512.png", "sizes": "512x512", "type": "image/png", "purpose": "maskable" },
        { "src": "512.png", "sizes": "512x512", "type": "image/png" }
    ] }));
    let (reports, found) = run(&mut host, &manifest);
    assert!(found.is_empty(), "{found:?}");
    assert_eq!(host.fetched().len(), 2, "one request per URL");
    assert_eq!(reports[1].state, "ok");
    assert_eq!(reports[1].width, Some(512));
    assert_eq!(reports[1].format.as_deref(), Some("image/png"));
    assert_eq!(reports[1].purpose, vec!["maskable"]);
}

#[test]
fn missing_sizes_broken_and_mislabelled_icons_are_findings() {
    let mut host = FakeHost::default();
    host.serve("https://a.test/app/a.png", 200, "image/png", &png(180, 180))
        .serve(
            "https://a.test/app/b.webp",
            200,
            "image/png",
            &png(192, 192),
        )
        .serve(
            "https://a.test/app/c.png",
            200,
            "text/html",
            b"<!doctype html>",
        );
    let manifest = manifest(&json!({ "icons": [
        { "src": "a.png", "sizes": "192x192" },
        { "src": "b.webp", "sizes": "192x192", "type": "image/webp" },
        { "src": "c.png", "sizes": "48x48" },
        { "src": "gone.png", "sizes": "96x96" },
        { "src": "https://cdn.test/d.png", "sizes": "64x64" },
        { "src": "data:image/png;base64,AAAA", "sizes": "16x16" }
    ] }));
    let (reports, found) = run(&mut host, &manifest);
    assert_eq!(
        found,
        vec![
            "manifest.icon_size pwa:manifest/icons/512px size=512",
            "icons.size_mismatch pwa:manifest/icons/0 src=a.png declared=192x192 actual=180x180",
            "icons.type_mismatch pwa:manifest/icons/1 src=b.webp declared=image/webp actual=image/png",
            "icons.not_image pwa:manifest/icons/2 src=c.png",
            "icons.unreachable pwa:manifest/icons/3 src=gone.png status=404",
            "fetch.other_origin pwa:manifest/icons/4 what=icon url=https://cdn.test/d.png",
        ]
    );
    let states: Vec<&str> = reports.iter().map(|r| r.state.as_str()).collect();
    assert_eq!(
        states,
        [
            "ok",
            "ok",
            "not_image",
            "unreachable",
            "other_origin",
            "skipped"
        ]
    );
    assert!(!host.fetched().iter().any(|url| url.contains("cdn.test")));
}

#[test]
fn a_scalable_icon_covers_every_size() {
    let mut host = FakeHost::default();
    host.serve(
        "https://a.test/app/icon.svg",
        200,
        "image/svg+xml",
        b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>",
    );
    let manifest = manifest(&json!({ "icons": [
        { "src": "icon.svg", "sizes": "any", "type": "image/svg+xml" }
    ] }));
    let (_, found) = run(&mut host, &manifest);
    assert!(found.is_empty(), "{found:?}");
}
