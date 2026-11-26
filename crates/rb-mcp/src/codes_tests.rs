use std::collections::BTreeSet;

use serde_json::json;

use super::{LEGACY_FIELD, RENAMED, add_legacy, is_dotted, legacy};

#[test]
fn every_renamed_code_is_dotted_and_every_alias_flat_and_unique() {
    let mut dotted = BTreeSet::new();
    let mut flat = BTreeSet::new();
    for (code, old) in RENAMED {
        assert!(is_dotted(code), "{code} is not area.reason");
        assert!(!old.contains('.'), "{old} is no flat code");
        assert!(dotted.insert(*code), "{code} twice");
        assert!(flat.insert(*old), "{old} twice");
    }
}

#[test]
fn a_renamed_code_names_its_flat_alias_and_an_old_dotted_one_none() {
    assert_eq!(legacy("evaluate.disabled"), Some("evaluate_disabled"));
    assert_eq!(legacy("args.invalid"), Some("invalid_arguments"));
    assert_eq!(legacy("config.key_protected"), Some("config_key_protected"));
    assert_eq!(legacy("script.disabled"), Some("scripts_disabled"));
    assert_eq!(legacy("network.compose_disabled"), Some("compose_disabled"));
    assert_eq!(legacy("cdp.disabled"), None);
    assert_eq!(legacy("settings.rejected.protected"), None);
    assert_eq!(legacy("evaluate_disabled"), None);
}

#[test]
fn the_alias_joins_a_body_with_a_renamed_code_only() {
    let mut renamed = json!({ "error": "off", "code": "evaluate.disabled" });
    add_legacy(renamed.as_object_mut().expect("an object"));
    assert_eq!(renamed[LEGACY_FIELD], "evaluate_disabled");

    let mut kept = json!({ "error": "off", "code": "cdp.disabled" });
    add_legacy(kept.as_object_mut().expect("an object"));
    assert!(kept.get(LEGACY_FIELD).is_none());
}

#[test]
fn the_style_is_area_dot_reason() {
    assert!(is_dotted("view.not_found"));
    assert!(is_dotted("settings.rejected.protected"));
    assert!(!is_dotted("view_not_found"));
    assert!(!is_dotted("View.notFound"));
    assert!(!is_dotted("view."));
}

/// Every failure code rb-mcp and the app write as a literal (`ToolFailure::new` with a string)
/// is dotted; this file, which names the call, is left out.
#[test]
fn every_literal_failure_code_in_the_sources_is_dotted() {
    let crates = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the crates folder");
    let mut flat = Vec::new();
    for root in [crates.join("rb-mcp/src"), crates.join("rbrowser/src")] {
        let mut folders = vec![root];
        while let Some(folder) = folders.pop() {
            let Ok(entries) = std::fs::read_dir(&folder) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    folders.push(path);
                } else if path.extension().is_some_and(|ext| ext == "rs")
                    && !path.ends_with("codes_tests.rs")
                {
                    let text = std::fs::read_to_string(&path).unwrap_or_default();
                    for code in literal_codes(&text) {
                        if !is_dotted(code) {
                            flat.push(format!("{}: {code}", path.display()));
                        }
                    }
                }
            }
        }
    }
    assert!(flat.is_empty(), "flat codes: {flat:#?}");
}

/// The string literal each `ToolFailure::new(` opens with, if it opens with one.
fn literal_codes(text: &str) -> Vec<&str> {
    text.match_indices("ToolFailure::new(")
        .filter_map(|(at, call)| {
            let rest = text[at + call.len()..].trim_start();
            let rest = rest.strip_prefix('"')?;
            rest.split_once('"').map(|(code, _)| code)
        })
        .collect()
}
