//! Self-update at the start (RB-090-02, RB-090-14), with the Cargo feature `self-update`.
//!
//! * [`before_window`], before CEF starts: an update the journal records as interrupted is ended
//!   or taken back (`recover_at_start`), whatever `updates.enabled` says - a half-switched program
//!   folder is never left so; when the older version was put back and this is the newer program,
//!   the older one starts instead and this process ends. Then this window takes its share of the
//!   instance lock, which the updater waits on.
//! * [`on_start`], off the CEF UI thread on the core's runtime (never `Handle::current()`
//!   there): the switched update this version is the target of is proven (`confirm_started`, the
//!   health mark) whatever the settings say, superseded downloads are removed while no other
//!   window runs, and with `updates.enabled` on and `updates.check = onStart` the channel is
//!   checked (`flow.rs`).

use std::sync::Mutex;

use rb_ipc::NotificationLevel;
use rb_update::install::InstanceLock;
use rb_update::install::handover;
use rb_update::install::process::spawn_detached;
use rb_update::install::recover::{self, Recovery};

use super::flow::{Origin, VERSION};
use super::notices;
use crate::control::Core;

/// This window's share of the instance lock, held until the process ends.
static INSTANCE: Mutex<Option<InstanceLock>> = Mutex::new(None);

/// Exit code of a start whose program folder could not be put in order.
const EXIT_RECOVERY_FAILED: i32 = 1;

pub(super) fn before_window() -> Option<i32> {
    let data = rb_config::local_dir()?;
    let executable = match std::env::current_exe() {
        Ok(executable) => executable,
        Err(error) => {
            tracing::warn!(%error, "the running program is unknown; no update recovery");
            return None;
        }
    };
    match recover::recover_at_start(&data, &executable, VERSION) {
        Ok(Recovery::Continue) => {}
        Ok(Recovery::Restart(program)) => {
            let args: Vec<String> = std::env::args_os()
                .skip(1)
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect();
            let folder = program
                .parent()
                .map_or_else(|| program.clone(), std::path::Path::to_path_buf);
            let started = spawn_detached(&program, args.as_slice(), &folder, None);
            return Some(match started {
                Ok(_) => {
                    tracing::warn!(
                        "the update was taken back; the previous version starts instead"
                    );
                    0
                }
                Err(error) => {
                    tracing::error!(
                        error = format!("{error:#}"),
                        "the previous version could not be started"
                    );
                    EXIT_RECOVERY_FAILED
                }
            });
        }
        Err(error) => {
            // The program folder may hold two versions at once: no window on top of it.
            tracing::error!(
                error = format!("{error:#}"),
                "an interrupted update could not be put in order; see the manual recovery in docs/release.md"
            );
            eprintln!("rbrowser: an interrupted update could not be put in order: {error:#}");
            return Some(EXIT_RECOVERY_FAILED);
        }
    }
    match InstanceLock::share(&data) {
        Ok(Some(lock)) => *rb_util::lock(&INSTANCE) = Some(lock),
        Ok(None) => {
            tracing::debug!("an updater holds the instance lock; this window runs without a share")
        }
        Err(error) => tracing::warn!(
            error = format!("{error:#}"),
            "the instance lock cannot be shared"
        ),
    }
    None
}

/// How many other rBrowser windows (other projects) share this data folder now.
pub(super) fn other_windows() -> usize {
    rb_util::lock(&INSTANCE)
        .as_ref()
        .map_or(0, InstanceLock::others)
}

pub(super) fn on_start(core: &Core, check: bool) {
    let Some(core) = core.arc() else {
        return;
    };
    let Some(data) = rb_config::local_dir() else {
        tracing::warn!("no local data folder; self-update does nothing");
        return;
    };
    let runtime = core.runtime().clone();
    runtime.spawn_blocking(move || {
        // Only the program the update switched proves it, not a build sharing the data.
        let proven = match std::env::current_exe() {
            Ok(exe) => {
                recover::confirm_started(&data, &exe, VERSION).map_err(|error| format!("{error:#}"))
            }
            Err(error) => Err(format!("the running program is unknown: {error}")),
        };
        match proven {
            Ok(true) => {
                tracing::info!(version = VERSION, "the update to this version is proven");
                core.notify(rb_ipc::CoreToShell::Notification(
                    notices::plain(
                        NotificationLevel::Success,
                        "update.notice.installed",
                        &[("version", VERSION.to_owned())],
                    )
                    .0,
                ));
            }
            Ok(false) => {}
            Err(error) => tracing::warn!(%error, "the update journal could not be read"),
        }
        // Downloads no update needs any more; another window's offer may still use its own.
        if other_windows() == 0 {
            let removed = handover::prune_downloads(&data, &[]);
            if removed > 0 {
                tracing::info!(removed, "superseded update downloads removed");
            }
        }
        if check {
            super::check::check_now(&core, Origin::Start);
        }
    });
}
