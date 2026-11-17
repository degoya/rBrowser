use super::FileNameTime;
use crate::UserSettings;

#[test]
fn names_a_person_reads_are_in_local_time_unless_the_user_picks_utc() {
    // RB-090-69.
    assert_eq!(
        UserSettings::default().ui.file_name_time,
        FileNameTime::Local
    );
    let utc: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "fileNameTime": "utc" } }"#)
            .expect("parses");
    assert_eq!(utc.ui.file_name_time, FileNameTime::Utc);
    let local: UserSettings =
        serde_json::from_str(r#"{ "version": 1, "ui": { "fileNameTime": "local" } }"#)
            .expect("parses");
    assert_eq!(local.ui.file_name_time, FileNameTime::Local);
    assert!(
        serde_json::from_str::<UserSettings>(
            r#"{ "version": 1, "ui": { "fileNameTime": "cet" } }"#
        )
        .is_err()
    );
}
