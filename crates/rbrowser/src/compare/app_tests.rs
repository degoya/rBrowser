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
