//! RB-094-26: every settings link of the tree lands on its place in the registry the window
//! reads - the core's keys and every bundled plugin's (`plugins/*/settings.schema.json`). The
//! links are the table `crates/rbrowser/data/settings-links.json` (RB-094-21) and every notice
//! target in the sources, found as the settings lint finds them (`scripts/lib/settings_targets.py`:
//! `settings_key: Some(..)`, `warning_notification(.., Some(..))`, `"settings.open"` with a
//! `"key"`, the arms of a `fn settings_key`, and in TypeScript and Vue `openSettings`,
//! `settingsKey`, `setting-key` and a status line's `setting: { key }`). A key must exist with a
//! category and a section the window has, a section must hold keys, a category sections. A
//! secret key's link (RB-094-27) lands on a panel its plugin's manifest has.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use rb_settings::SettingsRegistry;
use regex::Regex;
use serde_json::Value;

use super::*;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The core's keys and those of every bundled plugin with a settings schema, by its manifest.
fn registry_with_every_plugin() -> SettingsRegistry {
    let mut schemas = Vec::new();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(repo().join("plugins"))
        .expect("plugins/")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.join("manifest.toml").is_file())
        .collect();
    dirs.sort();
    for dir in dirs {
        let bytes = std::fs::read(dir.join("manifest.toml")).expect("manifest");
        let manifest = rb_plugin_host::parse_manifest(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error:#}", dir.display()));
        if !manifest.settings {
            continue;
        }
        let path = dir.join(rb_plugin_host::SETTINGS_SCHEMA_MEMBER);
        let text = std::fs::read_to_string(&path).expect("settings schema");
        let schema: Value = serde_json::from_str(&text).expect("settings schema JSON");
        schemas.push((manifest.id.clone(), schema));
    }
    assert!(schemas.len() > 20, "{} plugin schemas", schemas.len());
    let (mut registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "the core schemas: {issues:?}");
    for (plugin, schema) in &schemas {
        let taken = registry.register_plugin_schema(plugin, schema);
        assert!(taken.is_ok(), "{plugin}: {taken:?}");
    }
    registry
}

/// Every panel of the bundled plugins, `(plugin, panel)`, by their manifests.
fn panels_of_every_plugin() -> BTreeSet<(String, String)> {
    let mut panels = BTreeSet::new();
    for entry in std::fs::read_dir(repo().join("plugins")).expect("plugins/") {
        let dir = entry.expect("an entry").path();
        let Ok(bytes) = std::fs::read(dir.join("manifest.toml")) else {
            continue;
        };
        let manifest = rb_plugin_host::parse_manifest(&bytes)
            .unwrap_or_else(|error| panic!("{}: {error:#}", dir.display()));
        for panel in &manifest.panels {
            panels.insert((manifest.id.clone(), panel.id.clone()));
        }
    }
    panels
}

/// Why `link` does not land on its place; `None` when it does. A secret key's place is a panel
/// of `panels`, never the window.
fn misses(
    registry: &SettingsRegistry,
    panels: &BTreeSet<(String, String)>,
    link: &str,
) -> Option<String> {
    if let Some(panel) = panel_of(registry, link) {
        let known = panels.contains(&(panel.plugin.to_owned(), panel.panel.to_owned()));
        return (!known).then(|| format!("no panel {}.{}", panel.plugin, panel.panel));
    }
    match resolve(registry, link) {
        None => Some("opens nothing".to_owned()),
        Some(Target::Key(key)) => {
            let Some(def) = registry.get(key) else {
                return Some(format!("{key} is not registered"));
            };
            rb_settings::section(def.category, &def.section)
                .is_none()
                .then(|| format!("{key}: no section {}", def.section))
        }
        Some(Target::Category(category)) => category
            .sections()
            .next()
            .is_none()
            .then(|| format!("category {} has no section", category.id())),
        Some(Target::Section { category, section }) => {
            let held = registry
                .iter()
                .any(|def| def.category == category && def.section == section);
            (!held).then(|| format!("section {}.{section} holds no key", category.id()))
        }
    }
}

/// The table's links, `text key -> target`.
fn table() -> Vec<(String, String)> {
    let table: Value =
        serde_json::from_str(include_str!("../../data/settings-links.json")).expect("JSON");
    table["links"]
        .as_object()
        .expect("links")
        .iter()
        .map(|(text, target)| {
            let target = target.as_str().expect("a target is a string");
            (format!("settings-links.json: {text}"), target.to_owned())
        })
        .collect()
}

/// The source files below `dir` with one of `suffixes`, as the lint reads them: no tests, no
/// build output, no dependencies.
fn sources(dir: &Path, suffixes: &[&str], out: &mut Vec<PathBuf>) {
    const SKIP: [&str; 6] = [
        "node_modules",
        "target",
        "dist",
        "tests",
        "test",
        "fixtures",
    ];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    paths.sort();
    for path in paths {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !SKIP.contains(&name) {
                sources(&path, suffixes, out);
            }
        } else if suffixes.iter().any(|suffix| name.ends_with(suffix))
            && !name.ends_with("_tests.rs")
            && !name.contains(".test.")
        {
            out.push(path);
        }
    }
}

const TARGET: &str = r#"(?:"(?P<lit>[^"\\]*)"|(?P<const>(?:[a-z_][a-z0-9_]*::)*[A-Z][A-Z0-9_]*))"#;

/// The `&str` constants of every Rust source, by file.
fn constants(files: &[(PathBuf, String)]) -> BTreeMap<PathBuf, BTreeMap<String, String>> {
    let pattern = Regex::new(r#"const (?P<name>[A-Z][A-Z0-9_]*): &str = "(?P<value>[^"\\]*)";"#)
        .expect("regex");
    files
        .iter()
        .map(|(path, text)| {
            let own = pattern
                .captures_iter(text)
                .map(|m| (m["name"].to_owned(), m["value"].to_owned()))
                .collect();
            (path.clone(), own)
        })
        .collect()
}

/// The values constant `name` used in `path` may have: the nearest definitions - the file, its
/// directory, its crate (or the crate a path names), the tree - as the lint reads them.
fn const_values(
    path: &Path,
    name: &str,
    defined: &BTreeMap<PathBuf, BTreeMap<String, String>>,
) -> Vec<String> {
    let root = repo();
    let mut segments: Vec<&str> = name.split("::").collect();
    let last = segments.pop().unwrap_or(name);
    let foreign = segments
        .first()
        .filter(|first| !matches!(**first, "crate" | "super" | "self"));
    let relative = path.strip_prefix(&root).unwrap_or(path);
    let crate_dir = match foreign {
        Some(first) => root.join("crates").join(first.replace('_', "-")),
        None => root.join(relative.iter().take(2).collect::<PathBuf>()),
    };
    let start = if foreign.is_some() { 2 } else { 0 };
    for tier in start..4 {
        let values: Vec<String> = defined
            .iter()
            .filter(|(p, _)| in_tier(tier, p, path, &crate_dir))
            .filter_map(|(_, own)| own.get(last).cloned())
            .collect();
        if !values.is_empty() {
            return values;
        }
    }
    Vec::new()
}

/// Whether `p` lies in tier `tier` around `path`: 0 the file, 1 its directory, 2 its crate, 3
/// the tree.
fn in_tier(tier: usize, p: &Path, path: &Path, crate_dir: &Path) -> bool {
    match tier {
        0 => p == path,
        1 => p.parent() == path.parent(),
        2 => p.starts_with(crate_dir),
        _ => true,
    }
}

/// Every notice target of the sources, `(where, target)`.
fn notice_targets() -> Vec<(String, String)> {
    let root = repo();
    let mut rust = Vec::new();
    for dir in ["crates", "plugins"] {
        sources(&root.join(dir), &[".rs"], &mut rust);
    }
    let rust: Vec<(PathBuf, String)> = rust
        .into_iter()
        .map(|path| {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            (path, text)
        })
        .collect();
    let defined = constants(&rust);
    let patterns = [
        format!(r"settings_key:\s*Some\(\s*{TARGET}"),
        format!(r"warning_notification\(\s*[^,()]+,\s*Some\(\s*{TARGET}"),
        format!(r#""settings\.open",\s*json!\(\{{\s*"key":\s*{TARGET}"#),
    ]
    .map(|pattern| Regex::new(&pattern).expect("regex"));
    let settings_key_fn =
        Regex::new(r"(?s)fn settings_key\(&self\)[^{]*\{(?P<body>.*?)\n    \}").expect("regex");
    let some_in_fn = Regex::new(&format!(r"Some\(\s*{TARGET}\s*\)")).expect("regex");
    let mut found = Vec::new();
    for (path, text) in &rust {
        let where_ = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        // Each hit as `(literal, constant)`, one of them set.
        let mut hits: Vec<(Option<String>, Option<String>)> = Vec::new();
        let mut take = |captures: &regex::Captures<'_>| {
            hits.push((
                captures.name("lit").map(|m| m.as_str().to_owned()),
                captures.name("const").map(|m| m.as_str().to_owned()),
            ));
        };
        for pattern in &patterns {
            pattern.captures_iter(text).for_each(|c| take(&c));
        }
        for body in settings_key_fn.captures_iter(text) {
            let inner = body.name("body").map_or("", |m| m.as_str());
            some_in_fn.captures_iter(inner).for_each(|c| take(&c));
        }
        for hit in hits {
            match hit {
                (Some(literal), _) => found.push((where_.clone(), literal)),
                (None, Some(name)) => {
                    let values = const_values(path, &name, &defined);
                    assert!(
                        !values.is_empty(),
                        "{where_}: the constant {name} does not read"
                    );
                    found.extend(values.into_iter().map(|value| (where_.clone(), value)));
                }
                (None, None) => {}
            }
        }
    }
    let mut pages = Vec::new();
    for dir in ["ui/src", "plugins"] {
        sources(&root.join(dir), &[".ts", ".vue"], &mut pages);
    }
    let page_patterns = [
        r"type:\s*'openSettings',\s*key:\s*'(?P<lit>[^']*)'",
        r"settingsKey:\s*'(?P<lit>[^']*)'",
        r"\bopenSettings\(\s*'(?P<lit>[^']*)'",
        r#"(?:^|[^:\w-])setting-key="(?P<lit>[^"]*)""#,
        r"setting:\s*\{\s*key:\s*'(?P<lit>[^']*)'",
    ]
    .map(|pattern| Regex::new(pattern).expect("regex"));
    for path in pages {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let where_ = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .display()
            .to_string();
        for pattern in &page_patterns {
            for captures in pattern.captures_iter(&text) {
                found.push((where_.clone(), captures["lit"].to_owned()));
            }
        }
    }
    found
}

#[test]
fn every_link_of_the_table_lands_on_its_place() {
    let registry = registry_with_every_plugin();
    let panels = panels_of_every_plugin();
    let links = table();
    assert!(links.len() > 50, "{} links", links.len());
    let missed: Vec<String> = links
        .iter()
        .filter_map(|(text, target)| {
            misses(&registry, &panels, target).map(|why| format!("{text} -> {target}: {why}"))
        })
        .collect();
    assert!(missed.is_empty(), "{missed:#?}");
    // The owner's three samples (RB-094-21/26) among them, each on its key.
    for key in [
        "api-client.benchRepeats",
        "portal.presentation.enabled",
        "engines.android.enabled",
    ] {
        assert!(links.iter().any(|(_, target)| target == key), "{key}");
        assert_eq!(resolve(&registry, key), Some(Target::Key(key)));
    }
    // RB-094-27: the private values open the API client at Variables, never the window.
    assert!(
        links
            .iter()
            .any(|(_, target)| target == "api-client.secrets")
    );
    assert_eq!(resolve(&registry, "api-client.secrets"), None);
    assert_eq!(
        panel_of(&registry, "api-client.secrets"),
        Some(PanelLink {
            plugin: "api-client",
            panel: "api-client",
            tab: Some("variables")
        })
    );
}

#[test]
fn every_notice_target_lands_on_its_place() {
    let registry = registry_with_every_plugin();
    let panels = panels_of_every_plugin();
    let targets = notice_targets();
    assert!(targets.len() > 50, "{} notice targets", targets.len());
    let missed: Vec<String> = targets
        .iter()
        .filter_map(|(where_, target)| {
            misses(&registry, &panels, target).map(|why| format!("{where_}: {target}: {why}"))
        })
        .collect();
    assert!(missed.is_empty(), "{missed:#?}");
    // Plugin keys are among them: the registry without them could not have opened these.
    let plugin_keys = targets
        .iter()
        .filter(|(_, target)| registry.plugin_of(target).is_some())
        .count();
    assert!(plugin_keys > 5, "{plugin_keys} plugin keys");
}
