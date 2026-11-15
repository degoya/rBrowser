//! "Save diagnostics..." (RB-080-73): `diagnostics.create` run from the shell - the button in
//! Settings > Advanced > Diagnostics and on About, the command palette, the shortcut leader
//! `, D` - asks where with the Save dialog (the Desktop and `rbrowser-diagnostics-<date>-
//! <time>.zip` proposed), writes the ZIP there, says so in the status bar with the path, and
//! tells the windows (`diagnosticsSaved`), so the settings show it with "Show in folder"
//! (`revealDiagnostics`, which opens only the ZIP saved last - never a path a page names).
//! Cancelling writes nothing. MCP keeps the fixed folder: it names its arguments and gets no
//! dialog (`rb_diag::command::handler`).
//!
//! A ZIP the dialog may replace is written beside it first and moved over it, so a failed write
//! never leaves the old one half overwritten.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use rb_diag::bundle::bundle_file_name;
use rb_ipc::{CoreToShell, Notification, NotificationLevel, PathKind};

use super::Core;
use crate::cdp::lock;
use crate::host::ui::PathPick;

/// The i18n key of the dialog's title.
pub const SAVE_TITLE_KEY: &str = "diagnostics.save_title";
/// The i18n key of the notice after the ZIP was saved; placeholder `{path}`.
pub const SAVED_KEY: &str = "diagnostics.saved";
/// The i18n key of the notice when it could not be written; placeholder `{reason}`.
pub const SAVE_FAILED_KEY: &str = "diagnostics.save_failed";

/// Writes the ZIP to a path that does not exist yet; the error as text for the person.
pub type WriteZip = Arc<dyn Fn(&Path) -> Result<(), String> + Send + Sync>;

/// The ZIP saved last, the one "Show in folder" opens.
static LAST: Mutex<Option<PathBuf>> = Mutex::new(None);

/// The file the dialog proposes: in `folder` (the Desktop, else the diagnostics folder), named
/// after `now`.
pub fn proposed(folder: &Path, now: SystemTime) -> PathBuf {
    folder.join(bundle_file_name(now))
}

/// The person's Desktop when there is one.
pub fn desktop() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(home).join("Desktop")).filter(|folder| folder.is_dir())
}

/// Writes the ZIP to `target` through a file beside it, moved over `target` once complete.
pub fn write_replacing(target: &Path, write: &WriteZip) -> Result<(), String> {
    let mut partial = target.as_os_str().to_owned();
    partial.push(".partial");
    let partial = PathBuf::from(partial);
    // A partial file of an earlier, failed save is ours to remove.
    let _ = std::fs::remove_file(&partial);
    write(&partial)?;
    std::fs::rename(&partial, target).map_err(|error| {
        let _ = std::fs::remove_file(&partial);
        error.to_string()
    })
}

/// The one `diagnosticsSaved` a request gets: sent by [`Answer::send`], or on drop - a dialog
/// callback dropped unasked is a cancel, a write that never finished (a panic) a failure - so the
/// settings' button never waits for ever.
struct Answer {
    core: Option<Arc<Core>>,
    /// What a drop says: `false` before a path was chosen (cancelled), `true` after.
    failed_on_drop: bool,
    sent: bool,
}

impl Answer {
    fn new(core: Option<Arc<Core>>) -> Answer {
        Answer {
            core,
            failed_on_drop: false,
            sent: false,
        }
    }

    fn send(mut self, path: Option<String>, failed: bool) {
        self.sent = true;
        if let Some(core) = &self.core {
            core.send(&saved(path, failed));
        }
    }
}

impl Drop for Answer {
    fn drop(&mut self) {
        if !self.sent
            && let Some(core) = &self.core
        {
            core.send(&saved(None, self.failed_on_drop));
        }
    }
}

/// Asks where to save, then saves on a blocking thread. `folder` is where the dialog opens.
pub fn ask(core: &Core, folder: &Path, write: WriteZip) {
    let answer = Answer::new(core.arc());
    let pick = PathPick {
        kind: PathKind::File,
        title: rb_i18n::t(SAVE_TITLE_KEY),
        save_as: Some(proposed(folder, SystemTime::now())),
    };
    core.ui().pick_path(
        pick,
        Box::new(move |chosen| {
            let mut answer = answer;
            let (Some(target), Some(core)) = (chosen, answer.core.clone()) else {
                answer.send(None, false);
                return;
            };
            answer.failed_on_drop = true;
            let worker = core.clone();
            drop(
                core.runtime()
                    .spawn_blocking(move || finish(&worker, &target, &write, answer)),
            );
        }),
    );
}

/// Writes to `target` and says how it went.
fn finish(core: &Core, target: &Path, write: &WriteZip, answer: Answer) {
    match write_replacing(target, write) {
        Ok(()) => {
            *lock(&LAST) = Some(target.to_path_buf());
            let path = target.to_string_lossy().into_owned();
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Success,
                message_key: SAVED_KEY.to_owned(),
                args: [("path".to_owned(), path.clone())].into(),
                settings_key: None,
            }));
            answer.send(Some(path), false);
        }
        Err(reason) => {
            tracing::warn!(target: rb_diag::target::CORE, %reason, "the diagnostics ZIP was not saved");
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Error,
                message_key: SAVE_FAILED_KEY.to_owned(),
                args: [("reason".to_owned(), rb_diag::redact::redact_text(&reason))].into(),
                settings_key: None,
            }));
            answer.send(None, true);
        }
    }
}

fn saved(path: Option<String>, failed: bool) -> CoreToShell {
    CoreToShell::DiagnosticsSaved { path, failed }
}

/// Shows the ZIP saved last in the file manager; `false` when none was saved.
pub fn reveal_last() -> bool {
    let Some(path) = lock(&LAST).clone() else {
        return false;
    };
    if let Err(error) = rb_diag::reveal(&path) {
        tracing::warn!(target: rb_diag::target::CORE, %error, "the saved diagnostics ZIP cannot be shown");
    }
    true
}

#[cfg(test)]
#[path = "diag_save_tests.rs"]
mod tests;
