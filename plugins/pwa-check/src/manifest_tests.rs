use serde_json::json;

use super::*;

fn parse(value: &serde_json::Value) -> Manifest {
    let manifest = Url::parse("https://a.test/app/manifest.webmanifest").expect("url");
    let document = Url::parse("https://a.test/app/index.html").expect("url");
    Manifest::parse(&value.to_string(), &manifest, &document).expect("a manifest")
}

#[test]
fn urls_resolve_against_the_manifest() {
    let manifest = parse(&json!({
        "name": "App", "start_url": "./?source=pwa", "scope": "./",
        "icons": [{ "src": "icons/192.png", "sizes": "192x192", "type": "image/png" }]
    }));
    assert_eq!(
        manifest.start_url.to_string(),
        "https://a.test/app/?source=pwa"
    );
    assert_eq!(manifest.scope.to_string(), "https://a.test/app/");
    assert!(manifest.start_url_declared && manifest.scope_declared);
    let icon = &manifest.icons[0];
    assert_eq!(
        icon.url.as_ref().map(ToString::to_string).as_deref(),
        Some("https://a.test/app/icons/192.png")
    );
    assert_eq!(icon.sizes.listed, vec![(192, 192)]);
    assert_eq!(icon.kind.as_deref(), Some("image/png"));
    assert_eq!(icon.purpose, vec!["any".to_owned()]);
}

#[test]
fn defaults_follow_the_spec() {
    let manifest = parse(&json!({ "short_name": "A" }));
    assert_eq!(
        manifest.start_url.to_string(),
        "https://a.test/app/index.html"
    );
    assert_eq!(manifest.scope.to_string(), "https://a.test/app/");
    assert!(!manifest.start_url_declared && !manifest.scope_declared);
    assert_eq!(manifest.name, None);
    assert_eq!(manifest.short_name.as_deref(), Some("A"));
}

#[test]
fn a_start_url_of_another_origin_is_ignored() {
    let manifest = parse(&json!({ "start_url": "https://b.test/" }));
    assert_eq!(
        manifest.start_url.to_string(),
        "https://a.test/app/index.html"
    );
    assert!(manifest.start_url_ignored);
}

#[test]
fn blank_strings_and_wrong_types_count_as_missing() {
    let manifest = parse(&json!({
        "name": "  ", "display": 3, "theme_color": "", "icons": "nope", "id": "/app/"
    }));
    assert_eq!(manifest.name, None);
    assert_eq!(manifest.display, None);
    assert_eq!(manifest.theme_color, None);
    assert!(manifest.icons.is_empty());
    assert_eq!(manifest.id.as_deref(), Some("/app/"));
}

#[test]
fn display_modes_count_through_display_override_too() {
    assert!(parse(&json!({ "display": "standalone" })).installable_display());
    assert!(parse(&json!({ "display": "minimal-ui" })).installable_display());
    assert!(!parse(&json!({ "display": "browser" })).installable_display());
    assert!(
        parse(&json!({ "display": "browser", "display_override": ["window-controls-overlay", "fullscreen"] }))
            .installable_display()
    );
}

#[test]
fn purposes_split_and_maskable_is_found() {
    let manifest = parse(&json!({ "icons": [
        { "src": "a.png", "sizes": "512x512", "purpose": "any maskable" },
        { "src": "data:image/png;base64,AAAA", "sizes": "48x48" }
    ] }));
    assert_eq!(manifest.icons[0].purpose, vec!["any", "maskable"]);
    assert!(manifest.icons[0].maskable());
    assert_eq!(
        manifest.icons[1].url, None,
        "only http(s) icons are fetched"
    );
}

#[test]
fn malformed_json_and_non_objects_are_errors() {
    let manifest = Url::parse("https://a.test/m.json").expect("url");
    assert!(matches!(
        Manifest::parse("{ name: 1 }", &manifest, &manifest),
        Err(ParseError::Json(_))
    ));
    assert!(matches!(
        Manifest::parse("[1]", &manifest, &manifest),
        Err(ParseError::NotObject)
    ));
}

#[test]
fn the_table_shows_every_member_briefly() {
    let long = "x".repeat(400);
    let manifest = parse(&json!({
        "name": long, "icons": [{ "src": "a.png" }], "prefer_related_applications": false
    }));
    let table: std::collections::BTreeMap<_, _> = manifest.table.iter().cloned().collect();
    assert_eq!(table["name"].chars().count(), MAX_CELL_CHARS + 3);
    assert_eq!(table["icons"], "[1]");
    assert_eq!(table["prefer_related_applications"], "false");
}
