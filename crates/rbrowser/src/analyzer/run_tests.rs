use rb_config::ProjectConfig;
use rb_plugin_host::{LoadReport, Registry};

use super::{AnalyzerError, detect, pages, resolved_widths, static_files, write};
use crate::analyzer::plugins::LoadedPlugins;
use crate::analyzer::source::{Collected, Sheet};

fn no_plugins() -> LoadedPlugins {
    LoadedPlugins {
        report: LoadReport::default(),
        registry: Registry::new(),
        schemas: Vec::new(),
        settings_counts: Default::default(),
    }
}

#[test]
fn pages_are_the_request_then_start_url_then_the_settings() {
    let mut config = ProjectConfig {
        start_url: Some("http://localhost:5173/".to_owned()),
        ..ProjectConfig::default()
    };
    config.analyzer.urls = vec![
        "http://localhost:5173/".to_owned(),
        "https://example.org/about".to_owned(),
        "about:blank".to_owned(),
        "file:///etc/passwd".to_owned(),
    ];
    assert_eq!(
        pages(
            &config,
            &[" http://localhost:5173/pricing ".to_owned()],
            true
        ),
        vec![
            "http://localhost:5173/pricing",
            "http://localhost:5173/",
            "https://example.org/about"
        ]
    );
    let many: Vec<String> = (0..20).map(|n| format!("http://h/{n}")).collect();
    assert_eq!(pages(&ProjectConfig::default(), &many, false).len(), 10);
    assert!(pages(&ProjectConfig::default(), &[], false).is_empty());
}

#[test]
fn a_file_page_counts_only_when_the_person_names_it_on_the_command_line() {
    let config = ProjectConfig {
        start_url: Some("file:///site/index.html".to_owned()),
        ..ProjectConfig::default()
    };
    let named = vec!["FILE:///site/about.html".to_owned()];
    assert_eq!(
        pages(&config, &named, true),
        vec!["FILE:///site/about.html"]
    );
    assert!(pages(&config, &named, false).is_empty(), "not in the app");
    assert!(
        pages(&config, &[], true).is_empty(),
        "never from the checked-in config"
    );
}

#[test]
fn no_stylesheet_is_a_collect_error() {
    let error = detect(
        &no_plugins(),
        &ProjectConfig::default(),
        &Collected::default(),
        &[],
        None,
    )
    .expect_err("nothing to read");
    assert!(matches!(error, AnalyzerError::Collect(_)));
    assert_eq!(error.message().0, "analyzer.error.collect");
}

#[test]
fn without_the_plugin_nothing_is_detected_or_written() {
    let mut collected = Collected::default();
    collected.push(Sheet {
        url: "u".to_owned(),
        text: "@media (min-width: 1px) {}".to_owned(),
        media: None,
    });
    let error = detect(
        &no_plugins(),
        &ProjectConfig::default(),
        &collected,
        &[],
        None,
    )
    .expect_err("no plugin");
    assert_eq!(error.message().0, "plugins.error.not_available");

    let root = tempfile::tempdir().expect("temp dir");
    let error = write(&no_plugins(), root.path(), &[], &[], false).expect_err("no plugin");
    assert!(matches!(error, AnalyzerError::Plugin(_)));
    assert!(!root.path().join(".rbrowser").exists());
}

#[test]
fn the_plugin_gets_the_widths_device_presets_give_the_existing_breakpoints() {
    let existing = serde_json::json!([
        { "id": "phone", "device": "iphone-15" },
        { "id": "desktop", "width": 1440, "height": 900 },
        { "id": "ghost", "device": "no-such-phone" },
        "not a breakpoint"
    ]);
    assert_eq!(resolved_widths(&existing), vec![393, 1440]);
}

#[test]
fn static_files_need_the_setting_a_project_and_the_plugin_s_grant() {
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::write(root.path().join("app.css"), "@import \"tailwindcss\";").expect("written");
    let config = ProjectConfig::default();
    assert!(config.analyzer.static_scan, "on by default");
    // No plugin loaded: nothing is read, whatever the config allows.
    assert!(static_files(&no_plugins(), &config, Some(root.path())).is_empty());
    assert!(static_files(&no_plugins(), &config, None).is_empty());
}

#[test]
fn a_project_file_alone_is_enough_to_ask_the_plugin() {
    let files = vec![crate::analyzer::project_files::ProjectFile {
        path: "tailwind.config.js".to_owned(),
        text: "module.exports = {}".to_owned(),
    }];
    let error = detect(
        &no_plugins(),
        &ProjectConfig::default(),
        &Collected::default(),
        &files,
        None,
    )
    .expect_err("no plugin");
    assert_eq!(error.message().0, "plugins.error.not_available");
}
