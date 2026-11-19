use super::*;

#[test]
fn an_existing_flow_is_refused_so_the_dialog_can_ask() {
    let (code, message) = flow_failure(&FlowFileError::Exists(".rbrowser/flows/login.ts".into()));
    assert_eq!(code, IpcErrorCode::Refused);
    assert!(message.contains(".rbrowser/flows/login.ts"), "{message}");
    let (code, _) = flow_failure(&FlowFileError::BadName("../x".into()));
    assert_eq!(code, IpcErrorCode::BadRequest);
    let (code, message) = flow_failure(&FlowFileError::Linked(".rbrowser/flows".into()));
    assert_eq!(code, IpcErrorCode::Refused);
    assert!(message.contains("link"), "{message}");
    let (code, _) = flow_failure(&FlowFileError::Io("disk full".into()));
    assert_eq!(code, IpcErrorCode::Internal);
}

#[test]
fn the_saved_notice_names_every_file_written() {
    let files = [
        RecordingExportFile {
            path: ".rbrowser/flows/login.ts".to_owned(),
            content: String::new(),
        },
        RecordingExportFile {
            path: ".rbrowser/out/exports/login/login.spec.ts".to_owned(),
            content: String::new(),
        },
    ];
    let CoreToShell::Notification(notice) = saved_notice(&files) else {
        panic!("a notification");
    };
    assert_eq!(notice.level, NotificationLevel::Info);
    assert_eq!(notice.message_key, SAVED_NOTICE);
    assert_eq!(
        notice.args.get("files").map(String::as_str),
        Some(".rbrowser/flows/login.ts, .rbrowser/out/exports/login/login.spec.ts")
    );
    // The export commands name their files the same way (RB-090-84).
    assert_eq!(
        saved_paths_notice(&[
            ".rbrowser/flows/login.ts",
            ".rbrowser/out/exports/login/login.spec.ts"
        ]),
        saved_notice(&files)
    );
}
