//! RB-080-58: every error code a plugin builds has a text, so the status bar, the log and an MCP
//! answer show a sentence. The plugins' sources are read for the codes they build, and each is
//! resolved the way the host does at run time: the plugin's `locales/en.json` is registered and
//! [`failure_message`] must answer a sentence, not the code.
//!
//! What the scan finds: a literal `"plugin.<id>.errors.<name>"` or `"plugins.error.<name>"`, any
//! literal key fragment `"errors.<name>"` (a `const`, `key("errors.<name>")`), `key("errors",
//! "<name>")`, and the helpers `error`/`failure`/`coded`/`failed(FailureKind::_, "<name>")` and
//! `own("<name>")`. Its limit: a code whose name is computed at run time (`format!` with a
//! variable, a name passed through several functions) is not seen; such a code still shows as
//! its key with its parameters and logs `a plugin failure has no text` once.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rb_plugin_types::{Failure, FailureKind};
use regex::Regex;

use super::{failure_message, failure_text};

fn plugins_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins")
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            sources(&path, out);
        } else if name.ends_with(".rs") && !name.ends_with("_tests.rs") && !name.starts_with("fake")
        {
            out.push(path);
        }
    }
}

/// One code a plugin builds: the keys any of which must have a text.
type Code = Vec<String>;

fn codes_of(plugin: &str, text: &str) -> BTreeSet<Code> {
    let literal = Regex::new(r#""((?:plugin\.[a-z0-9-]+|plugins)\.errors?\.[a-z0-9_]+)""#);
    let grouped = Regex::new(r#"\bkey\(\s*"(errors?)",\s*"([a-z0-9_]+)"\s*\)"#);
    let dotted = Regex::new(r#"\bkey\(\s*"(errors?\.[a-z0-9_]+)"\s*\)"#);
    let helper =
        Regex::new(r#"\b(?:coded|failure|error|failed)\(\s*FailureKind::\w+,\s*"([a-z0-9_.-]+)""#);
    let own = Regex::new(r#"\bown\(\s*"([a-z0-9_-]+)"\s*\)"#);
    let fragment = Regex::new(r#""(errors?\.[a-z0-9_]+)""#).expect("valid");
    let (literal, grouped, dotted, helper, own) = (
        literal.expect("valid"),
        grouped.expect("valid"),
        dotted.expect("valid"),
        helper.expect("valid"),
        own.expect("valid"),
    );
    let mut codes = BTreeSet::new();
    for found in literal.captures_iter(text) {
        codes.insert(vec![found[1].to_owned()]);
    }
    for found in grouped.captures_iter(text) {
        codes.insert(vec![format!("plugin.{plugin}.{}.{}", &found[1], &found[2])]);
    }
    for found in dotted.captures_iter(text) {
        codes.insert(vec![format!("plugin.{plugin}.{}", &found[1])]);
    }
    for found in helper.captures_iter(text) {
        let name = &found[1];
        codes.insert(
            if name.starts_with("plugin.") || name.starts_with("plugins.") {
                vec![name.to_owned()]
            } else if name.contains('.') {
                vec![format!("plugin.{plugin}.{name}")]
            } else {
                vec![
                    format!("plugin.{plugin}.errors.{name}"),
                    format!("plugin.{plugin}.error.{name}"),
                ]
            },
        );
    }
    for found in fragment.captures_iter(text) {
        codes.insert(vec![format!("plugin.{plugin}.{}", &found[1])]);
    }
    for found in own.captures_iter(text) {
        codes.insert(vec![format!("plugin.{plugin}.errors.{}", &found[1])]);
    }
    codes
}

#[test]
fn every_error_code_a_plugin_builds_has_a_text() {
    let mut found: BTreeMap<String, BTreeSet<Code>> = BTreeMap::new();
    let mut missing = Vec::new();
    let entries = std::fs::read_dir(plugins_dir()).expect("the plugins folder");
    for entry in entries.flatten() {
        let dir = entry.path();
        if !dir.join("manifest.toml").is_file() {
            continue;
        }
        let plugin = entry.file_name().to_string_lossy().into_owned();
        if let Ok(locale) = std::fs::read(dir.join("locales/en.json")) {
            rb_i18n::register_plugin_locale(&plugin, "en", &locale).expect("a plugin locale loads");
        }
        let mut files = Vec::new();
        sources(&dir.join("src"), &mut files);
        let codes = found.entry(plugin.clone()).or_default();
        for file in files {
            let text = std::fs::read_to_string(&file).expect("a plugin source reads");
            codes.extend(codes_of(&plugin, &text));
        }
        for code in codes.iter() {
            let known = code.iter().any(|key| {
                let failure = Failure::coded(FailureKind::Other, key);
                failure_message(&plugin, &failure) != failure_text(&failure)
            });
            if !known {
                missing.push(format!("{plugin}: {}", code.join(" or ")));
            }
        }
    }
    assert!(
        missing.is_empty(),
        "error codes without a text: {missing:#?}"
    );
    // The scan reads what it should: Lighthouse's codes of RB-080-58 among them.
    let lighthouse = found.get("lighthouse").expect("the lighthouse plugin");
    for name in ["no_browser", "no_sidecar", "sidecar_failed"] {
        assert!(
            lighthouse
                .iter()
                .any(|code| code[0] == format!("plugin.lighthouse.errors.{name}")),
            "{name}: {lighthouse:?}"
        );
    }
    let total: usize = found.values().map(BTreeSet::len).sum();
    assert!(total >= 60, "only {total} codes found");
}

#[test]
fn every_host_code_has_a_text() {
    for code in rb_plugin_types::codes::ALL {
        assert!(
            rb_i18n::core_catalogue().get(code).is_some(),
            "{code} has no text in locales/en.json"
        );
    }
}
