//! A run of chosen checks merged into the last run (RB-094-17): the findings panel's "Run
//! again" on one check or a selection - `runChecks` or `checks_run` with `plugins` - over the
//! same views and page as the run the panel shows replaces only those checks' findings,
//! failures and skips, and their rows in the progress details; every other check's part stays
//! ([`merge::plugins_run`], as a plugin's own run does since RB-094-14). A run of every check,
//! or over other views or another page, replaces the run as before.

use std::collections::{BTreeMap, BTreeSet};

use rb_ipc::{CheckJob, ChecksProgress, CoreToShell};
use rb_plugin_host::RunRequest;
use rb_plugin_types::FindingsFile;

use super::merge;

/// The run a run of chosen checks merges into, with the texts of its keys and its rows.
#[derive(Debug)]
pub struct Base {
    run: FindingsFile,
    messages: BTreeMap<String, String>,
    /// The rows of the last run's finished progress; `None` when it did not finish.
    jobs: Option<Vec<CheckJob>>,
    /// The checks the run replaces: those it runs.
    plugins: Vec<String>,
}

/// The run the panel shows (`last`, the message it got) with the texts of its keys, when it is
/// a finished one; `None` for a run still going or none.
#[must_use]
pub fn shown(last: Option<&CoreToShell>) -> Option<(FindingsFile, BTreeMap<String, String>)> {
    match last? {
        CoreToShell::Findings {
            run,
            messages,
            partial: false,
            ..
        } => serde_json::from_value(run.clone())
            .ok()
            .map(|file| (file, messages.clone())),
        _ => None,
    }
}

impl Base {
    /// The base of `request`, which runs `plugins`: `last` (the run shown, with its texts) when
    /// the request asked for chosen checks and `last` checked the same views and page; the rows
    /// of `progress` when it finished. `None` for a run of every check, other views, another
    /// page or no last run: the run then replaces it.
    #[must_use]
    pub fn of(
        last: Option<(FindingsFile, BTreeMap<String, String>)>,
        progress: Option<&ChecksProgress>,
        request: &RunRequest,
        plugins: &[String],
    ) -> Option<Self> {
        request.plugins.as_ref()?;
        let (run, messages) = last?;
        let checked: BTreeSet<u32> = run.views.iter().map(|view| view.id).collect();
        let asked: BTreeSet<u32> = request.views.iter().map(|view| view.id).collect();
        if checked != asked || run.url != request.url {
            return None;
        }
        Some(Self {
            run,
            messages,
            jobs: progress
                .filter(|progress| progress.finished)
                .map(|progress| progress.jobs.clone()),
            plugins: plugins.to_vec(),
        })
    }

    /// `file` - the run of the chosen checks, whole or so far - merged into the base, with the
    /// texts of both (`texts` gives a run's); `file` alone should its views differ.
    #[must_use]
    pub fn run(
        &self,
        file: &FindingsFile,
        texts: impl Fn(&FindingsFile) -> BTreeMap<String, String>,
    ) -> (FindingsFile, BTreeMap<String, String>) {
        match merge::plugins_run(&self.run, file, &self.plugins) {
            Some(merged) => {
                let mut messages = self.messages.clone();
                messages.extend(texts(file));
                (merged, messages)
            }
            None => (file.clone(), texts(file)),
        }
    }

    /// `rows` - the chosen checks' rows at the end of their run - in place of theirs among the
    /// last run's rows; `rows` alone when that run did not finish.
    #[must_use]
    pub fn jobs(&self, rows: Vec<CheckJob>) -> Vec<CheckJob> {
        match &self.jobs {
            Some(previous) => merge::replace_rows(previous, rows, &self.plugins),
            None => rows,
        }
    }
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
