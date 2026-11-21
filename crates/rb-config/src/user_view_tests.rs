use super::*;

#[test]
fn the_views_open_the_start_url_unless_the_person_asks_for_the_last_pages() {
    assert_eq!(StartupSettings::default().restore, StartupRestore::StartUrl);
    let last: StartupSettings =
        serde_json::from_str(r#"{ "restore": "last" }"#).expect("last is a choice");
    assert_eq!(last.restore, StartupRestore::Last);
    let start: StartupSettings =
        serde_json::from_str(r#"{ "restore": "startUrl" }"#).expect("startUrl is a choice");
    assert_eq!(start.restore, StartupRestore::StartUrl);
    assert!(
        serde_json::from_str::<StartupSettings>(r#"{ "restore": "everything" }"#).is_err(),
        "only the two choices"
    );
}

#[test]
fn the_choice_is_written_as_the_schema_names_it() {
    assert_eq!(
        serde_json::to_value(StartupRestore::Last).expect("serialises"),
        serde_json::json!("last")
    );
    assert_eq!(
        serde_json::to_value(StartupRestore::StartUrl).expect("serialises"),
        serde_json::json!("startUrl")
    );
}
