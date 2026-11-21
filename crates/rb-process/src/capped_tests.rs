use tokio::process::Command;

use super::*;
use crate::NoConsoleWindow;

fn cargo_version() -> Command {
    let mut command = Command::new("cargo");
    command
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .no_console_window();
    command
}

#[tokio::test]
async fn the_output_within_the_cap_is_answered_whole() {
    let output = output_capped(&mut cargo_version(), MAX_OUTPUT)
        .await
        .expect("cargo runs");
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.starts_with("cargo "), "{text}");
}

#[tokio::test]
async fn output_over_the_cap_is_an_error_not_a_cut() {
    let error = output_capped(&mut cargo_version(), 4)
        .await
        .expect_err("over the cap");
    assert_eq!(error.kind(), std::io::ErrorKind::OutOfMemory);
    assert!(error.to_string().contains("4 bytes"), "{error}");
}

#[test]
fn the_cap_is_a_mebibyte() {
    assert_eq!(MAX_OUTPUT, 1024 * 1024);
}
