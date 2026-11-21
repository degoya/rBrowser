//! macOS: the inventory from `xcrun` and `system_profiler`, a simulator's boot with `simctl` and
//! the Simulator app shown on it. Every command runs without a console window, with stdin
//! closed, killed with its handle, given a deadline and read up to [`rb_process::MAX_OUTPUT`]
//! bytes (RB-095-05). Linted and tested on a Mac only
//! (the CI job `macos`).

use std::process::Stdio;
use std::time::Duration;

use rb_process::NoConsoleWindow;
use tokio::process::Command;

use super::{FIND_SIMCTL, LIST_SIMULATORS, LIST_USB, boot_args, bootstatus_args, show_args};
use crate::error::SafariError;
use crate::ios::inventory::{Inventory, Simulator, parse_simctl, parse_usb};
use crate::ios::udid::{self, is_udid};

const XCRUN: &str = "/usr/bin/xcrun";
const SYSTEM_PROFILER: &str = "/usr/sbin/system_profiler";
const OPEN: &str = "/usr/bin/open";
/// How long a listing may take.
const LIST_TIMEOUT: Duration = Duration::from_secs(20);

/// What `program args` printed when it ended well within `timeout`; `None` otherwise.
async fn run<I, S>(program: &str, args: I, timeout: Duration) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<std::ffi::OsStr>,
{
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .kill_on_drop(true)
        .no_console_window();
    let output = rb_process::output_capped(&mut command, rb_process::MAX_OUTPUT);
    let output = tokio::time::timeout(timeout, output).await.ok()?.ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Xcode, its iOS simulators, the iPhones and iPads on USB.
pub(in crate::ios) async fn inventory() -> Inventory {
    let xcode = run(XCRUN, FIND_SIMCTL, LIST_TIMEOUT).await.is_some();
    let simulators = if xcode {
        run(XCRUN, LIST_SIMULATORS, LIST_TIMEOUT)
            .await
            .map(|output| parse_simctl(&output))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let devices = run(SYSTEM_PROFILER, LIST_USB, LIST_TIMEOUT)
        .await
        .map(|output| parse_usb(&output))
        .unwrap_or_default();
    Inventory {
        xcode,
        simulators,
        devices,
    }
}

/// Boots `simulator` unless it runs, shows it in the Simulator app and waits until it is up.
pub(in crate::ios) async fn boot(
    simulator: &Simulator,
    timeout: Duration,
) -> Result<(), SafariError> {
    if !is_udid(&simulator.udid) {
        return Err(SafariError::Launch(
            "the simulator's id is not a UDID".to_owned(),
        ));
    }
    let short = udid::short(&simulator.udid);
    let wait = async {
        if !simulator.booted {
            // "Unable to boot device in current state: Booted" is fine: bootstatus decides.
            run(XCRUN, boot_args(&simulator.udid), timeout).await;
        }
        if run(OPEN, show_args(&simulator.udid), LIST_TIMEOUT)
            .await
            .is_none()
        {
            tracing::debug!(simulator = %short, "the Simulator app did not open");
        }
        run(XCRUN, bootstatus_args(&simulator.udid), timeout).await
    };
    match tokio::time::timeout(timeout, wait).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(SafariError::Launch(rb_i18n::t_with(
            "engines.ios.bootFailed",
            &[("simulator", &simulator.label())],
        ))),
        Err(_) => Err(SafariError::Launch(rb_i18n::t_with(
            "engines.ios.bootTimeout",
            &[
                ("simulator", &simulator.label()),
                ("seconds", &timeout.as_secs().to_string()),
            ],
        ))),
    }
}
