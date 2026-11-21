//! macOS: the version of a Safari bundle, read with `defaults` (it reads the binary and the XML
//! form of `Info.plist` alike). Linted and tested on a Mac only (the CI job `macos`).

use std::process::Stdio;

use rb_process::NoConsoleWindow;
use tokio::process::Command;

use crate::driver::Edition;

/// `defaults read <bundle>/Contents/Info CFBundleShortVersionString`, as text.
pub(super) async fn bundle_version(edition: Edition) -> Option<String> {
    let info = edition.bundle().join("Contents").join("Info");
    let mut command = Command::new("/usr/bin/defaults");
    command
        .arg("read")
        .arg(info)
        .arg("CFBundleShortVersionString")
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .no_console_window();
    let output = rb_process::output_capped(&mut command, rb_process::MAX_OUTPUT)
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}
