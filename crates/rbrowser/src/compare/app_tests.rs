use super::*;

#[test]
fn a_report_path_becomes_a_file_url() {
    assert_eq!(
        file_url(r"C:\work\site\.rbrowser\out\env-compare\r 1\report.html"),
        "file:///C:/work/site/.rbrowser/out/env-compare/r%201/report.html"
    );
    assert_eq!(
        file_url(r"\\wsl.localhost\Ubuntu\home\me\site\report.html"),
        "file://wsl.localhost/Ubuntu/home/me/site/report.html"
    );
    assert_eq!(file_url("/home/me/a#b.html"), "file:///home/me/a%23b.html");
}

#[test]
fn the_host_runs_the_four_tools() {
    assert_eq!(
        TOOLS,
        [
            "env_compare",
            "env_compare_result",
            "env_compare_live",
            "env_compare_cancel"
        ]
    );
}

/// RB-090-85: a comparison names its start with its number of shots, linked to its settings;
/// the end follows as `envcompare.notice.finished`.
#[test]
fn a_started_comparison_names_itself() {
    let started = Started {
        run_id: "r1".to_owned(),
        folder: "C:\\site\\.rbrowser\\out\\env-compare\\r1".to_owned(),
        shots: 12,
        diffs: 6,
    };
    let CoreToShell::Notification(notification) = started_notice(&started) else {
        panic!("a notification");
    };
    assert_eq!(notification.message_key, STARTED_KEY);
    assert_eq!(notification.level, NotificationLevel::Info);
    assert_eq!(notification.args["shots"], "12");
    assert_eq!(notification.settings_key.as_deref(), Some("env-compare"));
}
