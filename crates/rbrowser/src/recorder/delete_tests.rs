use super::*;

#[test]
fn the_recording_being_recorded_or_playing_stays() {
    let recording = refusal("login", Some("login"), None).expect_err("recording");
    assert!(
        recording.contains("being recorded") && recording.contains("recorder.stop"),
        "{recording}"
    );
    let playing = refusal("login", None, Some("login")).expect_err("playing");
    assert!(
        playing.contains("playing") && playing.contains("flows.cancel"),
        "{playing}"
    );
    // Another recording running or playing is no reason.
    assert_eq!(refusal("old", Some("login"), Some("checkout")), Ok(()));
    assert_eq!(refusal("old", None, None), Ok(()));
}

#[test]
fn the_notice_names_the_recording_and_why_it_stayed() {
    let CoreToShell::Notification(done) = outcome_notice("old", &Ok(())) else {
        panic!("a notice");
    };
    assert_eq!(done.level, NotificationLevel::Info);
    assert_eq!(done.message_key, DELETED_NOTICE);
    assert_eq!(done.args.get("name").map(String::as_str), Some("old"));
    let CoreToShell::Notification(failed) =
        outcome_notice("old", &Err("no recording named \"old\"".to_owned()))
    else {
        panic!("a notice");
    };
    assert_eq!(failed.level, NotificationLevel::Error);
    assert_eq!(failed.message_key, FAILED_NOTICE);
    assert!(failed.args.get("error").is_some_and(|e| e.contains("old")));
}

#[test]
fn the_command_is_the_persons_in_the_recorder_group() {
    assert_eq!(DELETE, "recorder.delete");
    let def = rb_commands::builtin_features::definitions()
        .into_iter()
        .find(|def| def.id.as_str() == DELETE)
        .expect("defined");
    assert_eq!(def.group, rb_commands::Group::Flows);
    assert_eq!(def.default_bindings[0].to_string(), "Ctrl+Shift+Space F M");
    assert!(matches!(def.mcp, rb_commands::McpCoverage::Omitted(_)));
}
