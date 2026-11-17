use std::path::{Path, PathBuf};

use rb_bidi::launcher::{Installed, Kind};
use serde_json::json;

use super::*;

fn found(path: &str, kind: Kind, version: Option<&str>) -> Installed {
    Installed {
        path: PathBuf::from(path),
        kind,
        version: version.map(str::to_owned),
    }
}

const RELEASE: &str = r"C:\Program Files\Mozilla Firefox\firefox.exe";
const DEVELOPER: &str = r"C:\Program Files\Firefox Developer Edition\firefox.exe";

fn two() -> Vec<Installed> {
    vec![
        found(RELEASE, Kind::Release, Some("134.0")),
        found(DEVELOPER, Kind::Developer, None),
    ]
}

#[test]
fn the_field_offers_every_installation_in_search_order_with_a_label() {
    let data = field_data(&two(), None, |_| true);
    assert_eq!(data.options.len(), 2);
    assert_eq!(data.options[0].value, RELEASE);
    assert_eq!(data.options[1].value, DEVELOPER);
    assert!(
        data.options[0].label.contains("134.0") && data.options[0].label.contains(RELEASE),
        "{}",
        data.options[0].label
    );
    assert!(
        data.options[1].label.contains(DEVELOPER),
        "{}",
        data.options[1].label
    );
    assert_eq!(data.missing, None);
}

#[test]
fn a_configured_path_that_does_not_exist_is_reported_an_empty_one_never() {
    let exists = |path: &Path| path == Path::new(RELEASE);
    assert_eq!(
        field_data(&two(), Some(r"D:\gone\firefox.exe"), exists).missing,
        Some(r"D:\gone\firefox.exe".to_owned())
    );
    assert_eq!(field_data(&two(), Some(RELEASE), exists).missing, None);
    assert_eq!(field_data(&two(), Some("  "), exists).missing, None);
    assert_eq!(field_data(&[], None, exists).options, Vec::new());
}

#[test]
fn the_field_data_serializes_for_the_settings_window() {
    let value = serde_json::to_value(field_data(
        &[found(RELEASE, Kind::Release, Some("134.0"))],
        Some("/gone"),
        |_| false,
    ))
    .expect("json");
    assert_eq!(value["options"][0]["value"], json!(RELEASE));
    assert!(value["options"][0]["label"].is_string());
    assert_eq!(value["missing"], json!("/gone"));
    let value = serde_json::to_value(field_data(&[], None, |_| false)).expect("json");
    assert_eq!(value, json!({ "options": [] }), "no `missing` without one");
}

#[test]
fn the_list_names_path_kind_and_version_and_the_configured_path() {
    let value = list_json(&two(), Some(RELEASE), |_| true);
    assert_eq!(
        value,
        json!({
            "firefox": {
                "configured": RELEASE,
                "installed": [
                    { "path": RELEASE, "kind": "release", "version": "134.0" },
                    { "path": DEVELOPER, "kind": "developer" },
                ]
            }
        })
    );
    let value = list_json(&[], Some("/gone"), |_| false);
    assert_eq!(value["firefox"]["configured"], json!("/gone"));
    assert_eq!(value["firefox"]["missing"], json!(true));
    assert_eq!(value["firefox"]["installed"], json!([]));
    let value = list_json(&[], None, |_| false);
    assert_eq!(value["firefox"]["configured"], Value::Null);
    assert!(value["firefox"].get("missing").is_none());
}

#[test]
fn the_text_lines_of_the_command_line_list_one_installation_per_line() {
    let lines = text_lines(&two(), Some("/gone"), |_| false);
    assert_eq!(
        lines,
        vec![
            "configured\tmissing\t/gone".to_owned(),
            format!("release\t134.0\t{RELEASE}"),
            format!("developer\t-\t{DEVELOPER}"),
        ]
    );
    assert_eq!(
        text_lines(&two(), Some(RELEASE), |_| true)[0],
        format!("configured\tok\t{RELEASE}")
    );
    assert_eq!(text_lines(&[], None, |_| true), Vec::<String>::new());
}
