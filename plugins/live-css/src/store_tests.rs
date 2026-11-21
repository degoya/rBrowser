use rb_plugin_api::FailureKind;
use serde_json::json;

use super::{Access, DRAFT_PATH, SHEET_PATH, access, read, write};
use crate::draft::{Block, Draft};
use crate::testing::host;

fn draft() -> Draft {
    Draft {
        enabled: false,
        blocks: vec![Block {
            css: "body { outline: 3px solid red }".to_owned(),
            breakpoints: vec!["md".to_owned()],
        }],
    }
}

#[test]
fn trust_shows_in_the_probes_answer() {
    let mut trusted = host();
    assert_eq!(access(&mut trusted), Access::Trusted);
    let mut untrusted = host();
    untrusted.trusted = false;
    assert_eq!(access(&mut untrusted), Access::Untrusted);
    let mut none = host();
    none.no_project = true;
    assert_eq!(access(&mut none), Access::NoProject);
}

#[test]
fn the_refusals_name_trust_and_the_missing_project() {
    assert!(Access::Trusted.refusal().is_none());
    let untrusted = Access::Untrusted.refusal().expect("refused");
    assert_eq!(untrusted.kind, FailureKind::AccessDenied);
    assert_eq!(
        untrusted.code.as_deref(),
        Some("plugin.live-css.errors.untrusted")
    );
    assert_eq!(
        Access::NoProject.refusal().and_then(|f| f.code).as_deref(),
        Some("plugin.live-css.errors.no_project")
    );
}

#[test]
fn a_written_draft_reads_back_with_its_stylesheet_beside_it() {
    let mut host = host();
    write(&mut host, &draft(), "the sheet").expect("written");
    assert_eq!(
        host.draft(),
        json!({ "version": 1, "enabled": false,
                "blocks": [{ "css": "body { outline: 3px solid red }", "breakpoints": ["md"] }] })
    );
    assert_eq!(host.sheet_file(), "the sheet");
    assert_eq!(read(&mut host), draft());
}

#[test]
fn without_a_file_the_draft_is_new() {
    let mut host = host();
    assert_eq!(read(&mut host), Draft::default());
    assert!(host.logs.is_empty());
}

#[test]
fn an_unusable_file_is_left_out_and_logged() {
    for text in [
        "not json",
        r#"{"version": 2, "enabled": true, "blocks": []}"#,
        r#"{"version": 1, "blocks": []}"#,
        r#"{"version": 1, "enabled": true, "blocks": [{"css": "} a {"}]}"#,
        r#"{"version": 1, "enabled": true, "blocks": [{"css": "a{}", "breakpoints": [""]}]}"#,
    ] {
        let mut host = host();
        host.file(DRAFT_PATH, text);
        assert_eq!(read(&mut host), Draft::default(), "{text}");
        assert_eq!(host.logs.len(), 1, "{text}");
    }
}

#[test]
fn the_paths_lie_in_the_plugins_out_folder() {
    assert_eq!(DRAFT_PATH, ".rbrowser/out/live-css/draft.json");
    assert_eq!(SHEET_PATH, ".rbrowser/out/live-css/draft.css");
}
