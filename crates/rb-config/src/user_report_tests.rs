use crate::{UserSettings, annotate};

use super::*;

#[test]
fn a_report_goes_to_the_project_and_is_previewed_by_default() {
    let report = UserSettings::default().report;
    assert_eq!(report.folder, "");
    assert!(
        report.preview,
        "the person sees the texts before they are saved"
    );
    assert_eq!(report.chosen_folder(), None);
}

#[test]
fn the_person_chooses_a_folder_and_may_skip_the_preview() {
    let settings: UserSettings = serde_json::from_str(
        r#"{ "version": 1, "report": { "folder": " D:\\tickets ", "preview": false } }"#,
    )
    .expect("parses");
    assert_eq!(settings.report.chosen_folder(), Some("D:\\tickets"));
    assert!(!settings.report.preview);
    let blank: UserReportSettings = serde_json::from_str(r#"{ "folder": "   " }"#).expect("parses");
    assert_eq!(blank.chosen_folder(), None, "blank is the project's folder");
    assert!(blank.preview);
}

#[test]
fn folder_and_preview_are_protected_user_keys_and_no_project_keys() {
    for name in ["report.folder", "report.preview"] {
        let key = annotate::USER_KEYS
            .iter()
            .find(|key| key.key == name)
            .unwrap_or_else(|| panic!("{name} is a user key"));
        assert!(key.protected, "MCP may not set {name}");
        assert!(
            !annotate::PROJECT_KEYS.iter().any(|key| key.key == name),
            "a checked-in config cannot send reports elsewhere or skip the preview: {name}"
        );
    }
    assert!(
        annotate::USER_INPUTS
            .iter()
            .any(|input| input.path == "report.folder" && input.input.id() == "folder"),
        "the settings window offers the folder dialog"
    );
}
