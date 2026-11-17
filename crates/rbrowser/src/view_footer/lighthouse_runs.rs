//! The Lighthouse runs the Lighthouse bar shows (RB-090-73), read from the plugin's own folder
//! `.rbrowser/out/lighthouse/<run>/run.json` - the summary each run writes - only in an open,
//! trusted project, as the report window reads them (`crate::report_view::guard`): each run
//! folder walked from the project root with no link followed, a summary of at most
//! [`MAX_SUMMARY_BYTES`], the newest [`MAX_RUNS`] runs.
//!
//! **When.** At the start, when `ui.viewFooter.lighthouse` or the workspace trust changes, and
//! after every answer of the Lighthouse plugin - a run, a deleted run, the retention -
//! (`crate::report_view::take_noted`). A read costs two file times first ([`signature`]: the
//! plugin folder's, which a new or deleted run folder changes, and `latest.json`'s, which every
//! run writes); the runs are read again only when that mark changed. Off the calling thread,
//! so reads may run at once and end in any order: a [`Gate`] lets only the newest started read
//! publish, and keeps its mark only when it read every summary whole - a summary caught while it
//! is written is read again at the next trigger.
//!
//! **A click** on the bar runs the plugin's `open_report` with the bar's run and breakpoint
//! ([`open_report`]), so the report opens where the plugin's history opens it.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use rb_engine::ViewId;
use serde_json::json;

use super::lighthouse::{Measured, OPEN_REPORT, PLUGIN, parse_run};
use crate::control::Core;
use crate::report_view::guard;

/// The most runs read: the newest, as the plugin's history lists them.
pub const MAX_RUNS: usize = 200;
/// The largest summary read; the plugin writes a few KiB.
pub const MAX_SUMMARY_BYTES: u64 = 1024 * 1024;
/// The summary of the newest run, which every run writes (`plugins/lighthouse/src/run.rs`).
const LATEST: &str = "latest.json";

/// Which read may publish, and the mark of the runs published last.
static GATE: Mutex<Gate> = Mutex::new(Gate::new());

/// What tells whether the runs changed: the plugin folder's time (a run folder came or went)
/// and the time and size of `latest.json` (a run ended).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Signature {
    folder: Option<SystemTime>,
    latest: Option<(SystemTime, u64)>,
}

/// The plugin's out folder in the project at `root`.
fn folder(root: &Path) -> PathBuf {
    root.join(".rbrowser").join("out").join(PLUGIN)
}

/// Whether `name` is a run id, `YYYYMMDD-HHMMSS`.
fn is_run_id(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() == 15
        && bytes.iter().enumerate().all(|(at, byte)| {
            if at == 8 {
                *byte == b'-'
            } else {
                byte.is_ascii_digit()
            }
        })
}

/// The mark of the runs of the project at `root`; the default without the plugin's folder.
pub fn signature(root: &Path) -> Signature {
    let folder = folder(root);
    let of = |path: &Path| {
        let metadata = std::fs::metadata(path).ok()?;
        Some((metadata.modified().ok()?, metadata.len()))
    };
    Signature {
        folder: of(&folder).map(|(time, _)| time),
        latest: of(&folder.join(LATEST)),
    }
}

/// What one read of the runs found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Read {
    /// The breakpoints of the runs, newest run first.
    pub runs: Vec<Measured>,
    /// Every summary read was whole: none failed to be read or to parse (a summary caught while
    /// it is written). One left out for good - too large, behind a link, not there - counts as
    /// whole: reading it again changes nothing.
    pub whole: bool,
}

/// One run folder as read.
enum RunRead {
    Runs(Vec<Measured>),
    /// Left out for good.
    Skipped,
    /// Not read whole: read again at the next trigger.
    Partial,
}

/// The breakpoints of the newest [`MAX_RUNS`] runs of the project at `root`, newest first.
pub fn read(root: &Path) -> Read {
    read_at_most(root, MAX_RUNS)
}

/// [`read`] of the newest `most` runs that read.
fn read_at_most(root: &Path, most: usize) -> Read {
    let entries = match std::fs::read_dir(folder(root)) {
        Ok(entries) => entries,
        Err(error) => {
            return Read {
                runs: Vec::new(),
                whole: error.kind() == std::io::ErrorKind::NotFound,
            };
        }
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| is_run_id(name))
        .collect();
    ids.sort_unstable_by(|a, b| b.cmp(a));
    let mut whole = true;
    let runs = ids
        .into_iter()
        .filter_map(|id| match read_run(root, &id) {
            RunRead::Runs(runs) => Some(runs),
            RunRead::Skipped => None,
            RunRead::Partial => {
                whole = false;
                None
            }
        })
        .take(most)
        .flatten()
        .collect();
    Read { runs, whole }
}

/// The breakpoints of run `id`: its folder a real one below the plugin's (no link on the way),
/// its summary a real file of at most [`MAX_SUMMARY_BYTES`] that parses.
fn read_run(root: &Path, id: &str) -> RunRead {
    let Ok(folder) = guard::run_folder(root, PLUGIN, id) else {
        return RunRead::Skipped;
    };
    let summary = folder.join("run.json");
    let Ok(metadata) = std::fs::symlink_metadata(&summary) else {
        return RunRead::Skipped;
    };
    if !metadata.is_file() || metadata.len() > MAX_SUMMARY_BYTES {
        return RunRead::Skipped;
    }
    let Ok(bytes) = guard::read_bounded(&summary, MAX_SUMMARY_BYTES) else {
        return RunRead::Partial;
    };
    if serde_json::from_slice::<serde::de::IgnoredAny>(&bytes).is_err() {
        return RunRead::Partial;
    }
    RunRead::Runs(parse_run(id, &bytes))
}

/// Which read of the runs may publish (review of RB-090-73): reads start off the calling thread
/// and may end in any order. Each started read gets a number; only the newest started may
/// publish, so an older one ending last never puts back older runs. The mark of the runs is
/// kept only after a whole read that published; until then every trigger reads again.
#[derive(Debug, Default)]
pub struct Gate {
    /// The number of the newest read started.
    started: u64,
    /// The project and the mark of the runs published last by a whole read.
    published: Option<(PathBuf, Signature)>,
}

impl Gate {
    /// No read yet.
    pub const fn new() -> Self {
        Self {
            started: 0,
            published: None,
        }
    }

    /// A read of the project at `root` whose runs bear `mark` is to start: its number, or
    /// `None` when the runs of that mark were published already.
    pub fn begin(&mut self, root: &Path, mark: &Signature) -> Option<u64> {
        let seen = self
            .published
            .as_ref()
            .is_some_and(|(seen, seen_mark)| seen == root && seen_mark == mark);
        if seen {
            return None;
        }
        self.started += 1;
        Some(self.started)
    }

    /// Read `ticket` of `root` at `mark` ended (`whole`: every summary read whole): whether it
    /// may publish - only the newest started read; it keeps the mark when it was whole, and
    /// none otherwise, so the next trigger reads again.
    pub fn finish(&mut self, ticket: u64, root: &Path, mark: Signature, whole: bool) -> bool {
        if ticket != self.started {
            return false;
        }
        self.published = whole.then(|| (root.to_path_buf(), mark));
        true
    }

    /// No project to read (none open, or not trusted): every read under way is stale, and the
    /// mark goes.
    pub fn clear(&mut self) {
        self.started += 1;
        self.published = None;
    }
}

/// Reads the runs again for the Lighthouse bar when they changed, off the calling thread, and
/// hands them to the host; no runs outside an open, trusted project. Nothing while the bar is
/// off: switching it on reads them.
pub fn reload(core: &Core) {
    if !core.user_settings().ui.view_footer.lighthouse {
        return;
    }
    let host = core.host().clone();
    let root = guard::admit(host.project_root(), host.shell_state().trusted).ok();
    core.runtime().spawn_blocking(move || {
        let Some(root) = root else {
            // Under the gate, so no read under way publishes after it.
            let mut gate = rb_util::lock(&GATE);
            gate.clear();
            host.set_footer_lighthouse(Vec::new());
            return;
        };
        // The mark before the read: a change during it moves the mark the next trigger sees.
        let mark = signature(&root);
        let Some(ticket) = rb_util::lock(&GATE).begin(&root, &mark) else {
            return;
        };
        let read = read(&root);
        tracing::debug!(
            breakpoints = read.runs.len(),
            whole = read.whole,
            "the Lighthouse runs were read for the Lighthouse bar"
        );
        // Published under the gate: a newer read cannot slip in between its check and the host.
        let mut gate = rb_util::lock(&GATE);
        if gate.finish(ticket, &root, mark, read.whole) {
            host.set_footer_lighthouse(read.runs);
        }
    });
}

/// The report of the run the Lighthouse bar of `view` shows, through the plugin's
/// `open_report` (`plugin.lighthouse.open_report`); nothing without a bar.
pub fn open_report(core: &Core, view: ViewId) {
    let Some((run, breakpoint)) = core.host().footer_lighthouse_target(view) else {
        return;
    };
    let args = json!({ "run": run, "breakpoint": breakpoint });
    core.run_command_async(OPEN_REPORT, args, |result| {
        if let Err(error) = result {
            tracing::warn!(%error, "the Lighthouse bar's report did not open");
        }
    });
}

#[cfg(test)]
#[path = "lighthouse_runs_tests.rs"]
mod tests;
