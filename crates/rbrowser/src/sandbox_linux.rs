//! The Chromium sandbox on Linux (RB-097-05, ADR 0001 addendum): before CEF starts, whether a
//! sandbox can start at all, so a start that would end in Chromium's `No usable sandbox!` abort,
//! with no window and, from the desktop menu, no terminal to read it in, says instead what to
//! do. Chromium takes the user namespaces when the kernel lets it, else the setuid helper
//! `chrome-sandbox` beside the program (root's, mode 4755: the deb and rpm install it so, an
//! unpacked tarball cannot). Ubuntu 23.10 and later restrict the namespaces with AppArmor, which
//! a profile may lift for one program - nothing short of trying finds that out, so that case
//! starts and leaves the verdict to Chromium.
//!
//! The decision is pure ([`verdict`], [`helper_state`], tested on every platform); [`probe`]
//! reads `/proc/sys` and the helper's metadata, on Linux only.

use std::ffi::OsString;
use std::path::Path;

/// The setuid helper's file name beside the program, as CEF ships it.
pub const HELPER: &str = "chrome-sandbox";

/// What the kernel and the helper say, as read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Probe {
    /// `/proc/sys/kernel/unprivileged_userns_clone` (Debian's switch), when there.
    pub unprivileged_userns_clone: Option<String>,
    /// `/proc/sys/user/max_user_namespaces`, when there.
    pub max_user_namespaces: Option<String>,
    /// `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` (Ubuntu), when there.
    pub apparmor_restrict_unprivileged_userns: Option<String>,
    /// The helper beside the program.
    pub helper: Helper,
}

/// The setuid helper `chrome-sandbox` beside the program.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Helper {
    /// Not there.
    #[default]
    Missing,
    /// Root's, setuid, writable by root alone: Chromium can use it.
    Setuid,
    /// There, but not usable (an unpacked tarball: the user's, no setuid bit).
    NotSetuid,
}

/// Which sandbox Chromium will find.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The user namespaces.
    Namespaces,
    /// The setuid helper.
    SetuidHelper,
    /// The namespaces are restricted by AppArmor and there is no setuid helper: a profile may
    /// still grant them; Chromium finds out.
    AppArmorRestricted,
    /// Neither: Chromium would abort.
    Unavailable,
}

impl Verdict {
    /// Whether the start stops here, with [`message`], instead of in Chromium's abort.
    #[must_use]
    pub fn stops_start(self) -> bool {
        self == Verdict::Unavailable
    }
}

/// The verdict for `probe`. Only a clear `0` turns the namespaces off and only a clear `1`
/// restricts them; anything else is left to Chromium.
#[must_use]
pub fn verdict(probe: &Probe) -> Verdict {
    let is = |value: &Option<String>, wanted: &str| value.as_deref().map(str::trim) == Some(wanted);
    let namespaces =
        !is(&probe.unprivileged_userns_clone, "0") && !is(&probe.max_user_namespaces, "0");
    let restricted = is(&probe.apparmor_restrict_unprivileged_userns, "1");
    if namespaces && !restricted {
        Verdict::Namespaces
    } else if probe.helper == Helper::Setuid {
        Verdict::SetuidHelper
    } else if namespaces {
        Verdict::AppArmorRestricted
    } else {
        Verdict::Unavailable
    }
}

/// The helper's state from its owner's uid and its mode (`None`: no file). Usable only when root
/// owns it, the setuid bit is set and neither the group nor others may write it.
#[must_use]
pub fn helper_state(owner_and_mode: Option<(u32, u32)>) -> Helper {
    const SETUID: u32 = 0o4000;
    const GROUP_OR_OTHER_WRITE: u32 = 0o022;
    match owner_and_mode {
        None => Helper::Missing,
        Some((0, mode)) if mode & SETUID != 0 && mode & GROUP_OR_OTHER_WRITE == 0 => Helper::Setuid,
        Some(_) => Helper::NotSetuid,
    }
}

/// Whether the arguments are those of one of CEF's subprocesses (`--type=renderer`, `zygote`,
/// `gpu-process`, ...): on Linux the program itself is every subprocess, and those must not take
/// the browser's file descriptors (the CDP pipe) or check the sandbox again.
pub fn is_subprocess(args: impl IntoIterator<Item = OsString>) -> bool {
    args.into_iter()
        .skip(1)
        .any(|arg| arg.to_str().is_some_and(|arg| arg.starts_with("--type=")))
}

/// The text the user reads when no sandbox can start: the helper's path, the two commands that
/// make it usable, and the way to start without the sandbox.
#[must_use]
pub fn message(program_dir: &Path) -> String {
    let helper = program_dir.join(HELPER).display().to_string();
    rb_i18n::t_with("startup.linuxSandbox.unavailable", &[("helper", &helper)])
}

/// Reads the kernel's switches and the helper beside `program_dir`.
#[cfg(target_os = "linux")]
#[must_use]
pub fn probe(program_dir: &Path) -> Probe {
    use std::os::unix::fs::MetadataExt as _;
    let read = |path: &str| std::fs::read_to_string(path).ok();
    let helper = std::fs::metadata(program_dir.join(HELPER))
        .ok()
        .map(|meta| (meta.uid(), meta.mode()));
    Probe {
        unprivileged_userns_clone: read("/proc/sys/kernel/unprivileged_userns_clone"),
        max_user_namespaces: read("/proc/sys/user/max_user_namespaces"),
        apparmor_restrict_unprivileged_userns: read(
            "/proc/sys/kernel/apparmor_restrict_unprivileged_userns",
        ),
        helper: helper_state(helper),
    }
}

#[cfg(test)]
#[path = "sandbox_linux_tests.rs"]
mod tests;
