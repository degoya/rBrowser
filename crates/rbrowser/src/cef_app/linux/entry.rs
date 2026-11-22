//! The Linux entry (RB-097-05): `rbrowser` itself, and every CEF subprocess it starts - the same
//! executable runs as `--type=zygote`, `renderer`, `gpu-process` and the others, and those leave
//! through [`super::run`] at once. Only the browser process makes the CDP pipe, first, before
//! anything else opens a file: a subprocess's descriptors 3 and up are Chromium's own. Then the
//! sandbox check ([`sandbox_stops`]): a sandbox that cannot start says how to fix it instead of
//! ending in Chromium's abort.

use std::path::Path;

use cef::*;

use super::{message_box, pipe, run};
use crate::sandbox_linux::{Verdict, message, probe, verdict};

/// The exit code when no Chromium sandbox can start.
const EXIT_NO_SANDBOX: i32 = 3;

/// Entry of `rbrowser` on Linux: with the sandbox unless `--no-sandbox` or
/// `engines.chromium.sandbox` turns it off.
pub fn run_plain() -> i32 {
    let _ = api_hash(sys::CEF_API_VERSION_LAST, 0);
    if !crate::sandbox_linux::is_subprocess(std::env::args_os()) {
        pipe::reserve();
    }
    let args = args::Args::new();
    let Some(command_line) = args.as_cmd_line() else {
        return 1;
    };
    run(
        args.as_main_args(),
        &command_line,
        std::ptr::null_mut(),
        true,
    )
}

/// `Some(exit code)` when neither the user namespaces nor the setuid helper beside `exe` can
/// give Chromium its sandbox: the reason goes to the log and the message, and the app ends. An
/// AppArmor restriction alone is only logged - a profile may lift it, Chromium finds out.
pub fn sandbox_stops(exe: &Path) -> Option<i32> {
    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    let found = verdict(&probe(dir));
    tracing::info!(?found, "the Chromium sandbox on Linux");
    match found {
        Verdict::Unavailable => {
            let text = message(dir);
            tracing::error!("no Chromium sandbox can start: {text}");
            message_box::error(&crate::start_failure::title(), &text);
            Some(EXIT_NO_SANDBOX)
        }
        Verdict::AppArmorRestricted => {
            tracing::warn!(
                "AppArmor restricts the user namespaces and chrome-sandbox is not setuid; \
                 Chromium starts only if a profile allows rbrowser"
            );
            None
        }
        Verdict::Namespaces | Verdict::SetuidHelper => None,
    }
}
