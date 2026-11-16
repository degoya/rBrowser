use super::*;

const CEF_BUILD: &str = "156.0.26+g8ea0c73+chromium-156.0.8078.88";

fn build(commit: &'static str, built: &'static str, cef: &'static str) -> Build<'static> {
    Build {
        version: "0.1.0",
        commit,
        built,
        cef,
    }
}

#[test]
fn the_cef_build_splits_into_cef_and_chromium() {
    assert_eq!(
        split_cef(CEF_BUILD),
        ("156.0.26+g8ea0c73".to_owned(), "156.0.8078.88".to_owned())
    );
}

#[test]
fn a_cef_build_without_chromium_keeps_what_it_names() {
    assert_eq!(
        split_cef("156.0.26"),
        ("156.0.26".to_owned(), UNKNOWN.to_owned())
    );
    assert_eq!(split_cef(""), (UNKNOWN.to_owned(), UNKNOWN.to_owned()));
    assert_eq!(
        split_cef("+chromium-1.2"),
        ("+chromium-1.2".to_owned(), UNKNOWN.to_owned())
    );
}

#[test]
fn the_facts_name_version_commit_build_time_platform_and_engines() {
    let facts = facts_from(
        build("f98c7fa1", "2026-11-10T12:00:00Z", CEF_BUILD),
        &Dirs::default(),
    );
    assert_eq!(facts.version, "0.1.0");
    assert_eq!(facts.commit, "f98c7fa1");
    assert_eq!(facts.built.as_deref(), Some("2026-11-10T12:00:00Z"));
    assert_eq!(facts.platform.as_deref(), Some(platform().as_str()));
    assert_eq!(facts.cef, "156.0.26+g8ea0c73");
    assert_eq!(facts.chromium, "156.0.8078.88");
    // Detected by RB-040-02 and RB-040-03; absent until then.
    assert_eq!(facts.firefox, None);
    assert_eq!(facts.firefox_path, None);
    assert_eq!(facts.webkit, None);
    assert_eq!(facts.node, None);
}

#[test]
fn an_unknown_stamp_reads_as_unknown_or_absent() {
    let facts = facts_from(build("unknown", "unknown", ""), &Dirs::default());
    assert_eq!(facts.commit, UNKNOWN);
    assert_eq!(facts.built, None);
    assert_eq!(facts.cef, UNKNOWN);
}

#[test]
fn the_api_versions_name_the_plugin_contract_and_the_config_schema() {
    let facts = facts_from(BUILD, &Dirs::default());
    assert_eq!(
        facts.apis.get("plugin").map(String::as_str),
        Some("rbrowser:plugin@0.1.0")
    );
    assert_eq!(
        facts.apis.get("configSchema"),
        Some(&rb_config::CURRENT_VERSION.to_string())
    );
}

#[test]
fn the_folders_are_named_only_when_known() {
    let dirs = Dirs {
        data: Some("C:\\Users\\a\\AppData\\Roaming\\rBrowser".to_owned()),
        local: None,
        logs: Some("C:\\Users\\a\\AppData\\Local\\rBrowser\\logs".to_owned()),
    };
    let facts = facts_from(BUILD, &dirs);
    let names: Vec<&str> = facts.dirs.keys().map(String::as_str).collect();
    assert_eq!(names, ["data", "logs"]);
}

#[test]
fn this_binary_carries_its_build_stamp() {
    let facts = facts();
    assert_eq!(facts.version, env!("CARGO_PKG_VERSION"));
    assert!(!facts.commit.is_empty());
}
