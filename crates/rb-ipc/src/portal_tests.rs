use serde_json::json;

use super::*;

/// RB-096-05: the presentation's link holds a token - its `Debug` output does not.
#[test]
fn the_presentation_link_hides_in_debug() {
    let presentation = PortalPresentation {
        enabled: true,
        consented: true,
        phase: PresentationPhase::Live,
        viewers: 1,
        max_viewers: 3,
        link: Some(PortalPairing {
            url: "https://quick-brown-fox.trycloudflare.com/__rbrowser/join#t=00ff00ff".to_owned(),
            size: 1,
            rows: vec!["1".to_owned()],
            expires_at_ms: 1,
        }),
        ..PortalPresentation::default()
    };
    let debug = format!("{presentation:?}");
    assert!(!debug.contains("00ff00ff"), "{debug}");
}

#[test]
fn the_presentation_reads_and_writes_camel_case() {
    let state = PortalState {
        presentation: Some(PortalPresentation {
            phase: PresentationPhase::Starting,
            max_viewers: 3,
            download: Some(PresentationDownload {
                version: "2026.11.1".to_owned(),
                size_mb: "55.5".to_owned(),
                license: "Apache-2.0".to_owned(),
                from: "github.com".to_owned(),
            }),
            ..PortalPresentation::default()
        }),
        ..PortalState::default()
    };
    let value = serde_json::to_value(&state).expect("serialises");
    assert_eq!(value["presentation"]["phase"], "starting");
    assert_eq!(value["presentation"]["maxViewers"], 3);
    assert_eq!(value["presentation"]["download"]["sizeMb"], "55.5");
    assert!(value["presentation"].get("link").is_none());
    let back: PortalState = serde_json::from_value(value).expect("parses");
    assert_eq!(back, state);
    let without: PortalState =
        serde_json::from_value(json!({ "enabled": false, "running": false, "devices": [] }))
            .expect("an older answer parses");
    assert_eq!(without.presentation, None);
}
