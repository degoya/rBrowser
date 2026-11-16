//! The memory of rBrowser's processes (RB-020-09): this process and every descendant - the
//! CEF renderers, the GPU and utility processes, the sidecar - summed as private memory (the
//! commit charge on Windows, Chromium's "memory footprint"; `RssAnon` on Linux). The memory
//! warning reads it; RB-060-10 may reuse it for app totals. [`sample`] blocks briefly: call it
//! from `spawn_blocking`.
//!
//! A descendant is found by its parent id. On Windows a process whose parent has exited keeps
//! the dead parent's id and drops out of every tree - a browser child Lighthouse's Chrome left
//! behind would never be counted. So the processes of rBrowser's job objects
//! ([`rb_process::adopted_pids`], the sidecar's whole tree) count too, wherever their parent
//! id points, and each process is named by its image, so the log says what they are
//! (RB-080-67).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// rBrowser's processes at one moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessSample {
    /// How many processes.
    pub processes: u32,
    /// Their private memory together, bytes.
    pub private_bytes: u64,
}

/// One process of the system's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProcEntry {
    /// Its id.
    pub pid: u32,
    /// Its parent's id.
    pub parent: u32,
    /// Its private memory; `None` when it could not be read.
    pub private_bytes: Option<u64>,
    /// When it started, in the platform's ticks; `None` when unknown. A process that started
    /// before its parent is not its child: the parent's id was reused (Windows keeps a dead
    /// parent's id in its children).
    pub started: Option<u64>,
    /// Its image (`chrome.exe`; the command name on Linux); `None` when unknown.
    pub name: Option<String>,
}

/// rBrowser's processes at one moment, with how many of them run each image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Census {
    /// The count and the memory.
    pub sample: ProcessSample,
    /// How many processes per image name (`?` for an unknown one).
    pub names: BTreeMap<String, u32>,
}

impl Census {
    /// The `count` most frequent images, `chrome.exe=300, rbrowser.exe=25`: most first, then
    /// by name.
    pub fn top(&self, count: usize) -> String {
        let mut names: Vec<(&String, &u32)> = self.names.iter().collect();
        names.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let mut line = String::new();
        for (name, processes) in names.into_iter().take(count) {
            if !line.is_empty() {
                line.push_str(", ");
            }
            let _ = write!(line, "{name}={processes}");
        }
        line
    }
}

/// `root` and its descendants in `entries`, summed; `None` when `root` is not listed. One map
/// of children, one walk; a process that started before its parent is left out with everything
/// below it.
pub fn tree_total(root: u32, entries: &[ProcEntry]) -> Option<ProcessSample> {
    tree_census(root, &BTreeSet::new(), entries).map(|census| census.sample)
}

/// [`tree_total`] plus the `adopted` processes (rBrowser's job objects) and theirs, wherever
/// their parent ids point, each counted once; with the images named.
pub fn tree_census(root: u32, adopted: &BTreeSet<u32>, entries: &[ProcEntry]) -> Option<Census> {
    let by_pid: BTreeMap<u32, &ProcEntry> =
        entries.iter().map(|entry| (entry.pid, entry)).collect();
    let root_entry = *by_pid.get(&root)?;
    let children = children_of(entries.iter().map(|entry| (entry.pid, entry.parent)));
    let mut seen = BTreeSet::from([root]);
    let mut queue = vec![root_entry];
    for pid in adopted {
        if let Some(&entry) = by_pid.get(pid)
            && seen.insert(*pid)
        {
            queue.push(entry);
        }
    }
    let mut census = Census {
        sample: ProcessSample {
            processes: 0,
            private_bytes: 0,
        },
        names: BTreeMap::new(),
    };
    while let Some(entry) = queue.pop() {
        census.sample.processes += 1;
        census.sample.private_bytes += entry.private_bytes.unwrap_or(0);
        let name = entry.name.clone().unwrap_or_else(|| "?".to_owned());
        *census.names.entry(name).or_default() += 1;
        let mine = children
            .get(&entry.pid)
            .into_iter()
            .flatten()
            .filter_map(|pid| by_pid.get(pid).copied())
            .filter(|child| !started_before(child, entry));
        for child in mine {
            if seen.insert(child.pid) {
                queue.push(child);
            }
        }
    }
    Some(census)
}

/// `root` and every process that descends from it in `links` (pid, parent): one map of
/// children, one walk; a cycle of reused ids ends.
pub fn descendants(root: u32, links: &[(u32, u32)]) -> BTreeSet<u32> {
    descendants_of(&BTreeSet::from([root]), links)
}

/// [`descendants`] of several `roots` at once, in one walk.
pub fn descendants_of(roots: &BTreeSet<u32>, links: &[(u32, u32)]) -> BTreeSet<u32> {
    let children = children_of(links.iter().copied());
    let mut seen = roots.clone();
    let mut queue: Vec<u32> = roots.iter().copied().collect();
    while let Some(pid) = queue.pop() {
        for &child in children.get(&pid).into_iter().flatten() {
            if seen.insert(child) {
                queue.push(child);
            }
        }
    }
    seen
}

/// The children of every parent in `links` (pid, parent); a process is never its own child.
fn children_of(links: impl Iterator<Item = (u32, u32)>) -> BTreeMap<u32, Vec<u32>> {
    let mut children: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for (pid, parent) in links.filter(|(pid, parent)| pid != parent) {
        children.entry(parent).or_default().push(pid);
    }
    children
}

/// Whether `child` started before `parent`, both known: then it is not `parent`'s child.
fn started_before(child: &ProcEntry, parent: &ProcEntry) -> bool {
    matches!((child.started, parent.started), (Some(child), Some(parent)) if child < parent)
}

/// Every process of the system as (pid, parent), for [`descendants`]; `None` where the
/// platform gives no process list. Blocks briefly (a snapshot); reads no process's details.
pub fn links() -> Option<Vec<(u32, u32)>> {
    Some(
        platform::list()?
            .into_iter()
            .map(|(pid, parent, _)| (pid, parent))
            .collect(),
    )
}

/// rBrowser's processes now, its job objects' included; `None` where the platform gives no
/// process list.
pub fn sample() -> Option<Census> {
    let me = std::process::id();
    let adopted: BTreeSet<u32> = rb_process::adopted_pids().into_iter().collect();
    let entries = entries_from(platform::list()?, Some((me, &adopted)), platform::read);
    tree_census(me, &adopted, &entries)
}

/// What is read of one process of ours.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    /// [`ProcEntry::private_bytes`].
    pub private_bytes: Option<u64>,
    /// [`ProcEntry::started`].
    pub started: Option<u64>,
    /// [`ProcEntry::name`].
    pub name: Option<String>,
}

/// The entries of `list` - (pid, parent, the platform's own data) - with `read`'s details only
/// for the processes of `scope`'s roots (this process and the adopted ones) and their
/// descendants; with no scope for none (review 6: never every process of the system).
pub fn entries_from<T>(
    list: Vec<(u32, u32, T)>,
    scope: Option<(u32, &BTreeSet<u32>)>,
    read: impl Fn(u32, &T) -> Details,
) -> Vec<ProcEntry> {
    let ours = scope.map(|(me, adopted)| {
        let links: Vec<(u32, u32)> = list
            .iter()
            .map(|(pid, parent, _)| (*pid, *parent))
            .collect();
        let mut roots = adopted.clone();
        roots.insert(me);
        descendants_of(&roots, &links)
    });
    list.into_iter()
        .map(|(pid, parent, raw)| {
            let details = if ours.as_ref().is_some_and(|ours| ours.contains(&pid)) {
                read(pid, &raw)
            } else {
                Details::default()
            };
            ProcEntry {
                pid,
                parent,
                private_bytes: details.private_bytes,
                started: details.started,
                name: details.name,
            }
        })
        .collect()
}

/// The image name in a NUL-terminated UTF-16 buffer (`PROCESSENTRY32W::szExeFile`); `None`
/// when it is empty.
#[cfg_attr(
    not(windows),
    allow(dead_code, reason = "read by the Windows list and the tests")
)]
pub fn image_name(raw: &[u16]) -> Option<String> {
    let end = raw.iter().position(|&unit| unit == 0).unwrap_or(raw.len());
    (end > 0).then(|| String::from_utf16_lossy(&raw[..end]))
}

/// The parent id in `/proc/<pid>/stat`: the second field after the command name, which may
/// itself hold parentheses.
pub fn parse_stat_parent(stat: &str) -> Option<u32> {
    let rest = &stat[stat.rfind(')')? + 1..];
    rest.split_whitespace().nth(1)?.parse().ok()
}

/// `RssAnon` of `/proc/<pid>/status`, bytes.
pub fn parse_rss_anon(status: &str) -> Option<u64> {
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("RssAnon:"))?;
    let kib: u64 = line.split_whitespace().next()?.parse().ok()?;
    Some(kib * 1024)
}

#[cfg(windows)]
use super::process_windows as platform;

#[cfg(target_os = "linux")]
mod platform {
    //! The list from `/proc`: each process's parent from `stat`; for ours the memory from
    //! `status` and the name from `comm`.

    use std::path::PathBuf;

    use super::{Details, parse_rss_anon, parse_stat_parent};

    /// Every process with its parent.
    pub fn list() -> Option<Vec<(u32, u32, ())>> {
        let mut list = Vec::new();
        for dir in std::fs::read_dir("/proc").ok()?.flatten() {
            let Some(pid) = dir.file_name().to_str().and_then(|name| name.parse().ok()) else {
                continue;
            };
            // A process may end between the listing and the read: it is skipped.
            let Ok(stat) = std::fs::read_to_string(dir.path().join("stat")) else {
                continue;
            };
            // Linux hands an orphan to init or a subreaper: its parent id is never stale.
            if let Some(parent) = parse_stat_parent(&stat) {
                list.push((pid, parent, ()));
            }
        }
        Some(list)
    }

    /// The memory and the name of one process.
    pub fn read(pid: u32, (): &()) -> Details {
        let dir = PathBuf::from(format!("/proc/{pid}"));
        Details {
            private_bytes: std::fs::read_to_string(dir.join("status"))
                .ok()
                .and_then(|status| parse_rss_anon(&status)),
            started: None,
            name: std::fs::read_to_string(dir.join("comm"))
                .ok()
                .map(|comm| comm.trim().to_owned())
                .filter(|comm| !comm.is_empty()),
        }
    }
}

#[cfg(not(any(windows, target_os = "linux")))]
mod platform {
    //! No process list (macOS comes with phase 2).

    pub fn list() -> Option<Vec<(u32, u32, ())>> {
        None
    }

    pub fn read(_pid: u32, (): &()) -> super::Details {
        super::Details::default()
    }
}

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;
