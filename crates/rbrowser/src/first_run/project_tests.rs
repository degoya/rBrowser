use std::path::Path;

use serde_json::json;

use rb_config::ProjectConfig;
use rb_ipc::SetupReasonCode;

use super::*;
use crate::cli::init::Analysed;

fn no_analyser(_: &Path, _: Option<&str>) -> Analysis {
    Err(NoProposal::new(Reason::NoPage, "no page"))
}

fn request<'a>(root: &'a Path, template: SetupTemplate, devices: &'a [String]) -> Preview<'a> {
    Preview {
        root,
        url: Some("http://localhost:5173"),
        template,
        devices,
    }
}

fn ids(config: &Value) -> Vec<String> {
    config["breakpoints"]
        .as_array()
        .expect("a list")
        .iter()
        .map(|view| view["id"].as_str().unwrap_or_default().to_owned())
        .collect()
}

#[test]
fn the_default_template_is_what_rbrowser_init_writes() {
    let dir = tempfile::tempdir().expect("temp dir");
    let Proposal { config, source, .. } = propose(
        &request(dir.path(), SetupTemplate::Default, &[]),
        no_analyser,
    )
    .expect("proposes");
    assert_eq!(source, SetupSource::Default);
    assert_eq!(
        config,
        crate::cli::init::minimal_config(Some("http://localhost:5173"))
    );
    // Nothing is written by a preview.
    assert!(!dir.path().join(".rbrowser").exists());
}

#[test]
fn the_analyser_proposes_and_falls_back_like_init() {
    let dir = tempfile::tempdir().expect("temp dir");
    let analysed = |_: &Path, url: Option<&str>| {
        let config: ProjectConfig = serde_json::from_value(json!({
            "version": rb_config::CURRENT_VERSION,
            "startUrl": url,
            "breakpoints": [{ "id": "sm", "width": 640, "height": 900 }]
        }))
        .map_err(|error| NoProposal::new(Reason::Invalid, error.to_string()))?;
        Ok(Analysed { config, sheets: 3 })
    };
    let found =
        propose(&request(dir.path(), SetupTemplate::Analyzer, &[]), analysed).expect("proposes");
    assert_eq!(found.source, SetupSource::Analyzer);
    assert_eq!(ids(&found.config), ["sm"]);
    // RB-091-08: what it read, for "N breakpoints in M stylesheets".
    assert_eq!(found.sheets, Some(3));
    assert_eq!(found.reason, None);
    let failing = |_: &Path, _: Option<&str>| {
        Err(NoProposal::new(Reason::PluginMissing, "not loaded")
            .with("code", "plugins.error.not_available"))
    };
    let fell_back =
        propose(&request(dir.path(), SetupTemplate::Analyzer, &[]), failing).expect("proposes");
    assert_eq!(fell_back.source, SetupSource::Default);
    assert_eq!(ids(&fell_back.config), ["mobile", "desktop"]);
    // RB-091-08: the fallback names its reason, never "nothing found" for every failure.
    let reason = fell_back.reason.expect("a reason");
    assert_eq!(reason.code, SetupReasonCode::PluginMissing);
    assert_eq!(
        reason.params.get("code").map(String::as_str),
        Some("plugins.error.not_available")
    );
}

#[test]
fn a_template_that_does_not_ask_the_analyser_names_no_reason() {
    let dir = tempfile::tempdir().expect("temp dir");
    let proposal = propose(
        &request(dir.path(), SetupTemplate::Default, &[]),
        no_analyser,
    )
    .expect("proposes");
    assert_eq!(proposal.reason, None);
    assert_eq!(proposal.sheets, None);
}

#[test]
fn the_analyser_needs_the_dev_servers_address() {
    let dir = tempfile::tempdir().expect("temp dir");
    let preview = Preview {
        url: None,
        ..request(dir.path(), SetupTemplate::Analyzer, &[])
    };
    let failure = propose(&preview, no_analyser).expect_err("refused");
    assert_eq!(failure.0, NEEDS_URL_KEY);
}

#[test]
fn chosen_devices_become_one_view_each() {
    let dir = tempfile::tempdir().expect("temp dir");
    let devices = vec!["iphone-15".to_owned(), "pixel-8".to_owned()];
    let Proposal { config, source, .. } = propose(
        &request(dir.path(), SetupTemplate::Devices, &devices),
        no_analyser,
    )
    .expect("proposes");
    assert_eq!(source, SetupSource::Devices);
    assert_eq!(ids(&config), ["iphone-15", "pixel-8"]);
    assert_eq!(config["breakpoints"][0]["device"], "iphone-15");
    assert_eq!(config["startUrl"], "http://localhost:5173");
}

#[test]
fn devices_must_be_known_and_chosen() {
    let dir = tempfile::tempdir().expect("temp dir");
    let none: Vec<String> = Vec::new();
    let failure = propose(
        &request(dir.path(), SetupTemplate::Devices, &none),
        no_analyser,
    )
    .expect_err("refused");
    assert_eq!(failure.0, NO_DEVICES_KEY);
    let unknown = vec!["toaster".to_owned()];
    let failure = propose(
        &request(dir.path(), SetupTemplate::Devices, &unknown),
        no_analyser,
    )
    .expect_err("refused");
    assert_eq!(failure.0, UNKNOWN_DEVICE_KEY);
    assert_eq!(failure.1.get("device").map(String::as_str), Some("toaster"));
}

#[test]
fn a_folder_with_a_config_or_no_folder_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    let missing = dir.path().join("nowhere");
    let failure =
        propose(&request(&missing, SetupTemplate::Default, &[]), no_analyser).expect_err("refused");
    assert_eq!(failure.0, NOT_FOLDER_KEY);
    rb_config::write_config_file(
        dir.path(),
        rb_config::ConfigFile::Project,
        &crate::cli::init::minimal_config(None),
    )
    .expect("writes");
    let failure = propose(
        &request(dir.path(), SetupTemplate::Default, &[]),
        no_analyser,
    )
    .expect_err("refused");
    assert_eq!(failure.0, EXISTS_KEY);
}

#[test]
fn only_web_addresses_are_a_start_url() {
    let dir = tempfile::tempdir().expect("temp dir");
    let preview = Preview {
        url: Some("file:///C:/site/index.html"),
        ..request(dir.path(), SetupTemplate::Default, &[])
    };
    let failure = propose(&preview, no_analyser).expect_err("refused");
    assert_eq!(failure.0, URL_KEY);
}

#[test]
fn the_shown_proposal_is_written_exactly_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut proposals = Proposals::default();
    let config = crate::cli::init::minimal_config(None);
    let id = proposals.keep(dir.path().to_path_buf(), config.clone());
    assert_eq!(
        proposals.take(id + 1).map(|(root, _)| root),
        None,
        "another number writes nothing"
    );
    let (root, kept) = proposals.take(id).expect("kept");
    let path = write(&root, &kept).expect("writes");
    assert_eq!(path, dir.path().join(".rbrowser").join("config.json"));
    assert_eq!(rb_config::read_json(&path).expect("reads"), config);
    assert!(proposals.take(id).is_none(), "taken once");
    // A second write never replaces a config.
    let failure = write(&root, &kept).expect_err("refused");
    assert_eq!(failure.0, EXISTS_KEY);
}
