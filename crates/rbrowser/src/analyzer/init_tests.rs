use rb_plugin_host::{LoadReport, Registry};

use super::init_config_with;
use crate::analyzer::fetch::Fetch;
use crate::analyzer::plugins::LoadedPlugins;

struct Offline;

impl Fetch for Offline {
    fn text(&self, url: &str, _limit: u64) -> Result<String, String> {
        Err(format!("{url}: connection refused"))
    }
}

struct Page;

impl Fetch for Page {
    fn text(&self, _url: &str, _limit: u64) -> Result<String, String> {
        Ok("<style>@media (min-width: 700px) {}</style>".to_owned())
    }
}

fn no_plugins() -> LoadedPlugins {
    LoadedPlugins {
        report: LoadReport::default(),
        registry: Registry::new(),
        schemas: Vec::new(),
        settings_counts: Default::default(),
    }
}

#[test]
fn without_a_page_the_cli_writes_its_minimal_config() {
    let root = tempfile::tempdir().expect("temp dir");
    let answer = init_config_with(root.path(), None, &no_plugins(), &Offline).expect("no error");
    assert!(answer.is_none());
    let ignored = init_config_with(root.path(), Some("about:blank"), &no_plugins(), &Offline);
    assert!(ignored.expect("no error").is_none());
}

#[test]
fn an_unreachable_page_or_a_missing_plugin_is_an_error() {
    let root = tempfile::tempdir().expect("temp dir");
    let offline = init_config_with(
        root.path(),
        Some("http://127.0.0.1:9/"),
        &no_plugins(),
        &Offline,
    )
    .expect_err("offline");
    assert!(offline.contains("connection refused"), "{offline}");
    let no_plugin = init_config_with(root.path(), Some("http://site.test/"), &no_plugins(), &Page)
        .expect_err("no plugin");
    assert!(
        no_plugin.contains("plugins.error.not_available"),
        "{no_plugin}"
    );
    assert!(
        !root.path().join(".rbrowser").exists(),
        "init_config writes nothing itself"
    );
}

#[test]
fn a_file_page_named_on_the_command_line_is_read() {
    let root = tempfile::tempdir().expect("temp dir");
    let error = init_config_with(
        root.path(),
        Some("file:///site/index.html"),
        &no_plugins(),
        &Page,
    )
    .expect_err("read, then no plugin");
    assert!(error.contains("plugins.error.not_available"), "{error}");
}
