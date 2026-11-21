use serde_json::json;

use super::*;
use crate::{CoreToShell, ShellToCore, parse_request};

#[test]
fn the_request_names_a_run_or_none() {
    assert_eq!(
        parse_request(r#"{ "type": "getSitemapGallery", "runId": "2026-11-11_10-00-00" }"#)
            .expect("parses"),
        ShellToCore::GetSitemapGallery(SitemapGalleryRequest {
            run_id: Some("2026-11-11_10-00-00".to_owned()),
        })
    );
    let latest = parse_request(r#"{ "type": "getSitemapGallery" }"#).expect("parses");
    assert_eq!(
        latest,
        ShellToCore::GetSitemapGallery(SitemapGalleryRequest::default())
    );
    assert_eq!(
        serde_json::to_value(&latest).expect("writes"),
        json!({ "type": "getSitemapGallery" })
    );
}

#[test]
fn the_answer_carries_the_run_flat_beside_its_type() {
    let mut shots = BTreeMap::new();
    shots.insert(
        "mobile".to_owned(),
        GalleryShot {
            state: "done".to_owned(),
            image: Some("rbrowser://sitemap/r1/index/mobile.png".to_owned()),
            ..GalleryShot::default()
        },
    );
    let page = SitemapGalleryPage {
        run_id: "r1".to_owned(),
        source: "http://localhost:3000/".to_owned(),
        state: "finished".to_owned(),
        thumbnail_width: 320,
        breakpoints: vec![GalleryBreakpoint {
            name: "mobile".to_owned(),
            label: "Mobile".to_owned(),
            width: 375,
            height: 812,
        }],
        pages: vec![GalleryPage {
            url: "http://localhost:3000/".to_owned(),
            status: Some(200),
            final_url: None,
            shots,
        }],
        total_pages: None,
    };
    let json = serde_json::to_value(CoreToShell::SitemapGallery(page)).expect("writes");
    assert_eq!(json["type"], "sitemapGallery");
    assert_eq!(json["thumbnailWidth"], 320);
    assert_eq!(
        json["pages"][0]["shots"]["mobile"]["image"],
        "rbrowser://sitemap/r1/index/mobile.png"
    );
    assert!(json["pages"][0].get("finalUrl").is_none());
    assert!(json["pages"][0]["shots"]["mobile"].get("error").is_none());
    assert!(
        json.get("totalPages").is_none(),
        "only when the pages were cut"
    );
}
