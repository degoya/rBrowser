//! K5 (RB-010-13, run-time part): `rbrowser init`'s proposal for the built Tailwind v3, Tailwind
//! v4 and Bootstrap fixtures (`tests/fixtures/analyzer/`), served over HTTP, through the real
//! WebAssembly component of `plugins/breakpoint-analyzer`: exactly the expected widths, and a
//! config the schema's types and rules accept. The cross-origin fixture reads a stylesheet of a
//! second origin. With the fixture folder as the project (RB-030-07), the static source reads its
//! Tailwind v3 config, Tailwind v4 entry or Bootstrap SCSS beside the stylesheets
//! (`expected-static.json`); `analyzer.static: false` gives the run-time proposal again.
//!
//! Needs the component: `scripts/build-plugins.sh --unsigned` (or the signed build) writes
//! `target/plugins/breakpoint-analyzer.wasm`; the test packages it unsigned and loads it in
//! development mode. Run with
//! `cargo nextest run -p rbrowser --test analyzer_e2e --run-ignored all`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use rb_config::UserSettings;
use rbrowser::analyzer::fetch::HttpFetch;
use rbrowser::analyzer::init::init_config_with;
use rbrowser::analyzer::plugins::{LoadedPlugins, load};
use serde_json::{Value, json};

/// The runtime `rbrowser init` enters before the analyser runs (RB-080-28: a plugin call on a
/// thread without one is an error). Hold the guard for the test.
fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime")
}

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The bundled folder with the component packaged unsigned, and the plugins loaded from it.
fn plugins(dir: &Path) -> LoadedPlugins {
    let component_path = repo().join("target/plugins/breakpoint-analyzer.wasm");
    let component = std::fs::read(&component_path).unwrap_or_else(|error| {
        panic!(
            "{}: {error} - run scripts/build-plugins.sh --unsigned first",
            component_path.display()
        )
    });
    let plugin = repo().join("plugins/breakpoint-analyzer");
    let manifest = std::fs::read(plugin.join("manifest.toml")).expect("manifest");
    let locale = std::fs::read(plugin.join("locales/en.json")).expect("locale");
    let package = rb_plugin_host::package_plugin(
        &manifest,
        &component,
        &[("locales/en.json".to_owned(), locale)],
        None,
    )
    .expect("packaged");
    std::fs::write(dir.join("breakpoint-analyzer.rbplugin"), package).expect("written");
    let mut user = UserSettings::default();
    user.plugins.development_mode = true;
    let loaded = load(dir, &user);
    assert_eq!(loaded.report.loaded.len(), 1, "the plugin loads");
    loaded
}

/// A one-folder HTTP server on 127.0.0.1; `{{SECOND_ORIGIN}}` in a page becomes `second`.
fn serve(dir: PathBuf, second: Option<String>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let origin = format!("http://{}", listener.local_addr().expect("address"));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            loop {
                let mut header = String::new();
                if reader.read_line(&mut header).map_or(true, |n| n == 0) || header == "\r\n" {
                    break;
                }
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/");
            let path = match path.trim_start_matches('/') {
                "" => "index.html",
                other => other,
            };
            let (status, body) = match std::fs::read_to_string(dir.join(path)) {
                Ok(body) => (
                    "200 OK",
                    body.replace("{{SECOND_ORIGIN}}", second.as_deref().unwrap_or_default()),
                ),
                Err(_) => ("404 Not Found", String::new()),
            };
            let mut stream = reader.into_inner();
            let _ = write!(
                stream,
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
        }
    });
    origin
}

fn fixtures() -> PathBuf {
    repo().join("tests/fixtures/analyzer")
}

fn expected(fixture: &str) -> Value {
    snapshot(fixture, "expected.json")
}

fn snapshot(fixture: &str, file: &str) -> Value {
    let text = std::fs::read_to_string(fixtures().join(fixture).join(file)).expect(file);
    serde_json::from_str(&text).expect("json")
}

/// The views of a config as in the snapshots: `[id, width, height]`.
fn views(config: &rb_config::ProjectConfig) -> Value {
    Value::Array(
        config
            .breakpoints
            .iter()
            .map(|breakpoint| json!([breakpoint.id, breakpoint.width, breakpoint.height]))
            .collect(),
    )
}

#[test]
#[ignore = "needs the component: scripts/build-plugins.sh --unsigned"]
fn k5_init_proposes_exactly_the_expected_widths_and_a_valid_config() {
    let runtime = runtime();
    let _entered = runtime.enter();
    let bundled = tempfile::tempdir().expect("temp dir");
    let plugins = plugins(bundled.path());
    for fixture in ["tailwind3", "tailwind4", "bootstrap", "cross-origin"] {
        let remote = serve(fixtures().join(fixture).join("remote"), None);
        let page = serve(fixtures().join(fixture), Some(remote));
        let project = tempfile::tempdir().expect("temp dir");
        let url = format!("{page}/");
        let config = init_config_with(project.path(), Some(&url), &plugins, &HttpFetch::default())
            .unwrap_or_else(|error| panic!("{fixture}: {error}"))
            .unwrap_or_else(|| panic!("{fixture}: no proposal"));
        assert_eq!(views(&config), expected(fixture)["all"], "{fixture}");
        assert_eq!(config.start_url.as_deref(), Some(url.as_str()));
        assert!(
            rb_config::check(&config).is_empty(),
            "{fixture}: {:?}",
            rb_config::check(&config)
        );
        assert_eq!(
            config.layout.cells.len(),
            config.breakpoints.len(),
            "{fixture}"
        );
        // Written and read back as the CLI does, it is a valid project.
        let value = serde_json::to_value(&config).expect("json");
        rb_config::write_config_file(project.path(), rb_config::ConfigFile::Project, &value)
            .expect("written");
        let loaded = rb_config::load_project(project.path()).expect("valid config");
        assert_eq!(loaded.config.breakpoints, config.breakpoints, "{fixture}");
    }
}

#[test]
#[ignore = "needs the component: scripts/build-plugins.sh --unsigned"]
fn the_wizard_writes_only_the_chosen_new_breakpoints() {
    let runtime = runtime();
    let _entered = runtime.enter();
    let bundled = tempfile::tempdir().expect("temp dir");
    let plugins = plugins(bundled.path());
    let project = tempfile::tempdir().expect("temp dir");
    let config = json!({
        "version": 1,
        "breakpoints": [{ "id": "phone", "width": 375, "height": 667, "touch": true }],
        "network": { "proxy": null }
    });
    rb_config::write_config_file(project.path(), rb_config::ConfigFile::Project, &config)
        .expect("written");
    let proposed: Vec<rb_ipc::ProposedBreakpoint> = serde_json::from_value(json!([
        { "id": "mobile", "label": "mobile", "width": 375, "height": 667, "isNew": false },
        { "id": "md", "label": "md", "width": 768, "height": 1024, "isNew": true },
        { "id": "lg", "label": "lg", "width": 1024, "height": 640, "isNew": true }
    ]))
    .expect("proposal");
    let added = rbrowser::analyzer::run::write(
        &plugins,
        project.path(),
        &proposed,
        &["md".to_owned()],
        false,
    )
    .expect("written");
    assert_eq!(added, vec!["md"]);
    let written: Value =
        rb_config::read_json(&rb_config::ConfigFile::Project.path(project.path())).expect("json");
    assert_eq!(written["breakpoints"][0], config["breakpoints"][0]);
    assert_eq!(written["breakpoints"][1]["id"], "md");
    assert_eq!(written["breakpoints"].as_array().map(Vec::len), Some(2));
    assert_eq!(written["network"], config["network"]);
}

#[test]
#[ignore = "needs the component: scripts/build-plugins.sh --unsigned"]
fn k5_init_with_the_static_source_proposes_the_project_s_breakpoints() {
    let runtime = runtime();
    let _entered = runtime.enter();
    let bundled = tempfile::tempdir().expect("temp dir");
    let plugins = plugins(bundled.path());
    for fixture in ["tailwind3", "tailwind4", "bootstrap"] {
        let page = serve(fixtures().join(fixture), None);
        let url = format!("{page}/");
        // The fixture folder is the project: its sources are read beside the stylesheets.
        let config = init_config_with(
            &fixtures().join(fixture),
            Some(&url),
            &plugins,
            &HttpFetch::default(),
        )
        .unwrap_or_else(|error| panic!("{fixture}: {error}"))
        .unwrap_or_else(|| panic!("{fixture}: no proposal"));
        assert_eq!(
            views(&config),
            snapshot(fixture, "expected-static.json")["all"],
            "{fixture}"
        );
        assert!(
            rb_config::check(&config).is_empty(),
            "{fixture}: {:?}",
            rb_config::check(&config)
        );
        let project = tempfile::tempdir().expect("temp dir");
        let value = serde_json::to_value(&config).expect("json");
        rb_config::write_config_file(project.path(), rb_config::ConfigFile::Project, &value)
            .expect("written");
        let loaded = rb_config::load_project(project.path()).expect("valid config");
        assert_eq!(loaded.config.breakpoints, config.breakpoints, "{fixture}");
    }
}

#[test]
#[ignore = "needs the component: scripts/build-plugins.sh --unsigned"]
fn analyzer_static_false_gives_the_run_time_proposal() {
    let runtime = runtime();
    let _entered = runtime.enter();
    let bundled = tempfile::tempdir().expect("temp dir");
    let plugins = plugins(bundled.path());
    let page = serve(fixtures().join("tailwind3"), None);
    let url = format!("{page}/");
    let project = tempfile::tempdir().expect("temp dir");
    std::fs::copy(
        fixtures().join("tailwind3/tailwind.config.js"),
        project.path().join("tailwind.config.js"),
    )
    .expect("copied");
    let propose = |root: &Path| {
        init_config_with(root, Some(&url), &plugins, &HttpFetch::default())
            .expect("proposal")
            .expect("some")
    };
    assert_eq!(
        views(&propose(project.path())),
        snapshot("tailwind3", "expected-static.json")["all"]
    );
    let off = json!({ "version": 1, "analyzer": { "static": false } });
    rb_config::write_config_file(project.path(), rb_config::ConfigFile::Project, &off)
        .expect("written");
    assert_eq!(
        views(&propose(project.path())),
        expected("tailwind3")["all"]
    );
}
