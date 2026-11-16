use serde_json::json;

use crate::rules::test_util::{landmark, page, param, short, with};
use crate::rules::*;
use crate::settings::Settings;

fn run(landmarks: serde_json::Value, outside: serde_json::Value) -> Vec<rb_plugin_api::Finding> {
    let outline = page(json!({
        "url": "https://site.test/",
        "landmarks": landmarks,
        "outside": outside
    }));
    findings(
        &[],
        1,
        &outline,
        &["landmarks".to_owned()],
        &[],
        &Settings::default(),
    )
}

#[test]
fn a_second_main_is_found_at_the_first_as_axe_reports_it() {
    let found = run(
        json!([
            landmark("body > main:nth-of-type(1)", "main", ""),
            landmark("body > main:nth-of-type(2)", "main", ""),
            with(
                landmark("body > div", "main", ""),
                json!({ "hidden": true })
            ),
        ]),
        json!([]),
    );
    assert_eq!(
        short(&found),
        ["landmark-main-multiple body > main:nth-of-type(1)"]
    );
    assert_eq!(param(&found[0], "count"), Some("2"));
}

#[test]
fn a_doubled_banner_and_contentinfo_are_found() {
    let found = run(
        json!([
            landmark("body > header:nth-of-type(1)", "banner", ""),
            landmark("body > header:nth-of-type(2)", "banner", ""),
            landmark("body > main", "main", ""),
            landmark("body > footer:nth-of-type(1)", "contentinfo", ""),
            landmark("body > footer:nth-of-type(2)", "contentinfo", ""),
        ]),
        json!([]),
    );
    assert_eq!(
        short(&found),
        [
            "landmark-banner-multiple body > header:nth-of-type(1)",
            "landmark-contentinfo-multiple body > footer:nth-of-type(1)"
        ]
    );
}

#[test]
fn a_banner_or_contentinfo_inside_another_landmark_is_found() {
    let found = run(
        json!([
            landmark("body > main", "main", ""),
            landmark("body > main > div", "banner", ""),
            landmark("body > main > aside", "complementary", ""),
            landmark("body > main > aside >>> footer", "contentinfo", ""),
            landmark("body > mainly", "contentinfo", ""),
        ]),
        json!([]),
    );
    assert_eq!(
        short(&found),
        [
            "landmark-contentinfo-multiple body > main > aside >>> footer",
            "landmark-banner-nested body > main > div",
            "landmark-contentinfo-nested body > main > aside >>> footer"
        ]
    );
    assert_eq!(param(&found[1], "outer"), Some("main"));
    assert_eq!(param(&found[2], "outer"), Some("complementary"));
}

#[test]
fn landmarks_of_one_role_without_a_name_that_tells_them_apart_are_found_once() {
    let found = run(
        json!([
            landmark("body > main", "main", ""),
            landmark("nav:nth-of-type(1)", "navigation", ""),
            landmark("nav:nth-of-type(2)", "navigation", ""),
            landmark("nav:nth-of-type(3)", "navigation", "Footer"),
            landmark("aside:nth-of-type(1)", "complementary", "Related"),
            landmark("aside:nth-of-type(2)", "complementary", "related"),
            landmark("section:nth-of-type(1)", "region", "Intro"),
            landmark("section:nth-of-type(2)", "region", "Contact"),
        ]),
        json!([]),
    );
    assert_eq!(
        short(&found),
        [
            "landmark-unique nav:nth-of-type(1)",
            "landmark-unique aside:nth-of-type(1)"
        ]
    );
    assert_eq!(param(&found[0], "role"), Some("navigation"));
    assert_eq!(param(&found[0], "count"), Some("2"));
    assert_eq!(param(&found[1], "name"), Some("Related"));
}

#[test]
fn visible_content_outside_every_landmark_is_found_with_axes_region_id() {
    let found = run(
        json!([landmark("body > main", "main", "")]),
        json!([
            with(
                json!({ "selector": "body > p", "path": "html > body > p", "visible": true }),
                json!({ "text": "Loose text" })
            ),
            json!({ "selector": "body > div", "path": "html > body > div", "hidden": true, "text": "Gone" }),
        ]),
    );
    assert_eq!(short(&found), ["content-outside-landmarks html > body > p"]);
    assert_eq!(param(&found[0], "text"), Some("Loose text"));
    let rule = RULES
        .iter()
        .find(|r| r.id == CONTENT_OUTSIDE_LANDMARKS)
        .expect("rule");
    assert_eq!(rule.shared, Some("content-outside-landmarks"));
}

#[test]
fn the_landmark_rules_share_their_ids_with_axe() {
    for (id, shared) in [
        (LANDMARK_MAIN_MISSING, "landmark-main-missing"),
        (LANDMARK_MAIN_MULTIPLE, "landmark-main-multiple"),
        (LANDMARK_BANNER_MULTIPLE, "landmark-banner-multiple"),
        (
            LANDMARK_CONTENTINFO_MULTIPLE,
            "landmark-contentinfo-multiple",
        ),
        (LANDMARK_BANNER_NESTED, "landmark-banner-nested"),
        (LANDMARK_CONTENTINFO_NESTED, "landmark-contentinfo-nested"),
        (LANDMARK_UNIQUE, "landmark-unique"),
    ] {
        let rule = RULES.iter().find(|r| r.id == id).expect("rule");
        assert_eq!(rule.shared, Some(shared), "{id}");
    }
}
