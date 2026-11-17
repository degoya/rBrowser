use super::*;
use crate::UserSettings;

#[test]
fn engine_status_shows_and_the_address_only_when_it_differs_by_default() {
    let settings = UserSettings::default();
    assert_eq!(
        settings.ui.cell_label,
        CellLabelSettings {
            engine_status: true,
            address: CellAddress::Differs,
        }
    );
}

#[test]
fn each_piece_is_switched_on_its_own() {
    let settings: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "cellLabel": { "address": "always" } } }"#)
            .expect("parses");
    assert!(
        settings.ui.cell_label.engine_status,
        "untouched keys keep their default"
    );
    assert_eq!(settings.ui.cell_label.address, CellAddress::Always);
}

#[test]
fn the_address_choices_are_written_in_lower_case() {
    for (choice, text) in [
        (CellAddress::Off, "\"off\""),
        (CellAddress::Differs, "\"differs\""),
        (CellAddress::Always, "\"always\""),
    ] {
        assert_eq!(serde_json::to_string(&choice).expect("writes"), text);
    }
}

#[test]
fn a_file_with_the_vitals_key_of_rb_090_27_keeps_its_choice() {
    // RB-090-71: the vitals left the label strip for a bar of their own (`ui.viewFooter.vitals`);
    // an older file that switched them off keeps the bar off (`retired::move_keys`), and the
    // old key is not written again.
    let mut value = serde_json::json!({
        "version": 1, "ui": { "cellLabel": { "vitals": false, "engineStatus": false } }
    });
    crate::retired::move_keys(&mut value);
    let settings: UserSettings = serde_json::from_value(value).expect("an older file loads");
    assert!(!settings.ui.cell_label.engine_status);
    assert!(
        !settings.ui.view_footer.vitals,
        "switched off, the bar stays off"
    );
    let written = serde_json::to_value(&settings).expect("writes");
    assert!(
        written["ui"]["cellLabel"].get("vitals").is_none(),
        "the old key is never written again: {written}"
    );
}
