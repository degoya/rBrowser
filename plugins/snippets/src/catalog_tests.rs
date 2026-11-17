use serde_json::json;

use super::{Access, LIBRARY_PATH, MAX_PER_SOURCE, Probe, Source, load, probe};
use crate::config::Settings;
use crate::fake_host::FakeHost;
use crate::header::World;
use crate::run::RunTarget;

fn host() -> FakeHost {
    let mut host = FakeHost::with_views(Vec::new());
    host.file(
        ".rbrowser/snippets/grid.js",
        "// @name Layout grid\n// @description Logs the grid\n// @world main\n// @target active\n// @timeoutMs 2000\nreturn 1",
    );
    host.file(".rbrowser/snippets/title.js", "return document.title");
    host.file(".rbrowser/snippets/readme.md", "not a snippet");
    host.file(".rbrowser/snippets/bad.js", "// @world page\nreturn 1");
    host
}

fn settings() -> Settings {
    Settings {
        library: vec![
            json!({ "name": "Title", "code": "return document.title" }),
            json!({ "name": "", "code": "skipped: no name" }),
            json!({ "name": "no code" }),
        ],
        ..Settings::default()
    }
}

fn names(catalog: &super::Catalog, source: Source) -> Vec<String> {
    catalog
        .snippets
        .iter()
        .filter(|snippet| snippet.source == source)
        .map(|snippet| snippet.name.clone())
        .collect()
}

#[test]
fn a_trusted_workspace_lists_the_project_and_the_saved_snippets() {
    let mut host = host();
    host.file(
        LIBRARY_PATH,
        &json!({ "version": 1, "snippets": [{ "name": "Saved", "code": "return 2" }] }).to_string(),
    );
    let catalog = load(&mut host, &settings());
    assert_eq!(catalog.project, Access::Loaded);
    assert_eq!(catalog.local, Access::Loaded);
    assert_eq!(names(&catalog, Source::User), ["Title"]);
    assert_eq!(
        names(&catalog, Source::Project),
        ["bad", "Layout grid", "title"]
    );
    assert_eq!(names(&catalog, Source::Local), ["Saved"]);
    let grid = catalog
        .find(Some(Source::Project), "Layout grid")
        .expect("found");
    assert_eq!(grid.world, World::Main);
    assert_eq!(grid.target, RunTarget::Active);
    assert_eq!(grid.timeout_ms, 2000);
    assert_eq!(grid.description, "Logs the grid");
    assert_eq!(grid.file.as_deref(), Some("grid.js"));
    let title = catalog.find(Some(Source::Project), "title").expect("found");
    assert_eq!(title.world, World::Isolated, "the default world");
    assert_eq!(title.timeout_ms, 5000);
}

#[test]
fn an_untrusted_workspace_reads_neither_the_project_nor_the_saved_snippets() {
    let mut host = host();
    host.trusted = false;
    host.file(
        LIBRARY_PATH,
        &json!({ "snippets": [{ "name": "Planted", "code": "steal()" }] }).to_string(),
    );
    let catalog = load(&mut host, &settings());
    assert_eq!(catalog.project, Access::Untrusted);
    assert_eq!(catalog.local, Access::Untrusted);
    assert_eq!(
        names(&catalog, Source::User),
        ["Title"],
        "the person's own still run"
    );
    assert!(names(&catalog, Source::Project).is_empty());
    assert!(names(&catalog, Source::Local).is_empty());
    let failure = catalog.find(None, "Layout grid").expect_err("blocked");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.project_untrusted")
    );
    let failure = catalog.find(None, "Planted").expect_err("blocked");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.project_untrusted")
    );
    let listed = catalog.to_json(false);
    assert_eq!(listed["project"], "untrusted");
    assert_eq!(listed["local"], "untrusted");
}

#[test]
fn the_probe_reads_the_hosts_answer() {
    let mut host = host();
    assert!(matches!(probe(&mut host), Probe::Trusted(names) if names.len() == 4));
    host.trusted = false;
    assert_eq!(probe(&mut host), Probe::Untrusted);
    host.no_project = true;
    assert_eq!(probe(&mut host), Probe::NoProject);
    let mut empty = FakeHost::with_views(Vec::new());
    assert_eq!(
        probe(&mut empty),
        Probe::Trusted(Vec::new()),
        "no folder yet"
    );
}

#[test]
fn the_project_source_can_be_switched_off() {
    let mut host = host();
    let settings = Settings {
        project_enabled: false,
        ..settings()
    };
    let catalog = load(&mut host, &settings);
    assert_eq!(catalog.project, Access::Disabled);
    assert!(names(&catalog, Source::Project).is_empty());
}

#[test]
fn a_bad_header_is_listed_and_never_runs() {
    let mut host = host();
    let catalog = load(&mut host, &settings());
    let listed = catalog.to_json(false);
    let bad = listed["snippets"]
        .as_array()
        .expect("list")
        .iter()
        .find(|entry| entry["name"] == "bad")
        .expect("listed")
        .clone();
    assert_eq!(bad["error"]["code"], "plugin.snippets.errors.header");
    assert_eq!(bad["error"]["params"]["tag"], "world");
    let failure = catalog.find(None, "bad").expect_err("invalid");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.invalid_snippet")
    );
}

#[test]
fn a_name_in_two_sources_needs_its_source() {
    let mut host = host();
    let settings = Settings {
        library: vec![json!({ "name": "title", "code": "return 1" })],
        ..Settings::default()
    };
    let catalog = load(&mut host, &settings);
    let failure = catalog.find(None, "title").expect_err("ambiguous");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.ambiguous")
    );
    assert_eq!(
        catalog
            .find(Some(Source::User), "title")
            .expect("found")
            .code,
        "return 1"
    );
    let failure = catalog
        .find(Some(Source::Local), "title")
        .expect_err("none saved");
    assert_eq!(
        failure.code.as_deref(),
        Some("plugin.snippets.errors.not_found")
    );
}

#[test]
fn a_second_snippet_of_a_name_is_marked() {
    let mut host = FakeHost::with_views(Vec::new());
    let settings = Settings {
        library: vec![
            json!({ "name": "same", "code": "return 1" }),
            json!({ "name": "same", "code": "return 2" }),
        ],
        ..Settings::default()
    };
    let catalog = load(&mut host, &settings);
    assert_eq!(
        catalog
            .find(Some(Source::User), "same")
            .expect("the first")
            .code,
        "return 1"
    );
    assert_eq!(
        catalog.snippets[1].error.as_ref().expect("marked")["code"],
        "plugin.snippets.errors.duplicate_name"
    );
}

#[test]
fn the_listing_carries_code_on_request_and_caps_the_sources() {
    let mut host = FakeHost::with_views(Vec::new());
    let library: Vec<_> = (0..MAX_PER_SOURCE + 5)
        .map(|n| json!({ "name": format!("s{n}"), "code": "return 1" }))
        .collect();
    let settings = Settings {
        library,
        ..Settings::default()
    };
    let catalog = load(&mut host, &settings);
    assert_eq!(catalog.snippets.len(), MAX_PER_SOURCE);
    assert!(catalog.to_json(false)["snippets"][0].get("code").is_none());
    assert_eq!(catalog.to_json(true)["snippets"][0]["code"], "return 1");
}

#[test]
fn without_a_project_only_the_persons_snippets_are_there() {
    let mut host = host();
    host.no_project = true;
    let catalog = load(&mut host, &settings());
    assert_eq!(catalog.project, Access::NoProject);
    assert_eq!(names(&catalog, Source::User), ["Title"]);
    assert_eq!(catalog.snippets.len(), 1);
}
