// RB-090-73: the Lighthouse runs on disk the bar reads - only the plugin's own run folders, no
// link followed, a summary of bounded size - and the mark that says whether they changed.

use std::fs;
use std::path::Path;

use serde_json::json;

use super::*;

fn out(root: &Path) -> std::path::PathBuf {
    root.join(".rbrowser").join("out").join("lighthouse")
}

fn write_run(root: &Path, id: &str, breakpoint: &str) {
    let folder = out(root).join(id);
    fs::create_dir_all(&folder).expect("run folder");
    let summary = json!({
        "runId": id, "startedAt": 1_794_925_920_u64,
        "breakpoints": [{
            "breakpoint": breakpoint, "url": "https://example.test/",
            "finalUrl": "https://example.test/",
            "scores": { "performance": 72, "accessibility": 96, "best-practices": 81, "seo": 100 },
            "report": format!(".rbrowser/out/lighthouse/{id}/{breakpoint}.html"),
        }],
    });
    fs::write(
        folder.join("run.json"),
        serde_json::to_vec(&summary).expect("json"),
    )
    .expect("summary");
}

#[test]
fn the_runs_are_read_from_the_plugins_run_folders_newest_first() {
    let root = tempfile::tempdir().expect("tempdir");
    write_run(root.path(), "20261117-100000", "md");
    write_run(root.path(), "20261117-120000", "sm");
    // Not a run folder: another name, a file, a folder without a summary.
    fs::create_dir_all(out(root.path()).join("notes")).expect("folder");
    fs::write(out(root.path()).join("latest.json"), b"{}").expect("latest");
    fs::create_dir_all(out(root.path()).join("20261117-130000")).expect("empty run");
    let runs = read(root.path()).runs;
    let names: Vec<(&str, &str)> = runs
        .iter()
        .map(|run| (run.run.as_str(), run.breakpoint.as_str()))
        .collect();
    assert_eq!(
        names,
        [("20261117-120000", "sm"), ("20261117-100000", "md")]
    );
}

#[test]
fn no_project_folder_reads_as_no_runs() {
    let root = tempfile::tempdir().expect("tempdir");
    assert!(read(root.path()).runs.is_empty());
    assert_eq!(signature(root.path()), Signature::default());
}

#[test]
fn a_summary_past_its_size_is_not_read() {
    let root = tempfile::tempdir().expect("tempdir");
    write_run(root.path(), "20261117-100000", "md");
    let summary = out(root.path()).join("20261117-100000").join("run.json");
    let mut bytes = fs::read(&summary).expect("summary");
    bytes.resize(usize::try_from(MAX_SUMMARY_BYTES).expect("size") + 1, b' ');
    fs::write(&summary, bytes).expect("grown");
    let read = read(root.path());
    assert!(read.runs.is_empty());
    assert!(
        read.whole,
        "too large stays too large: no reason to read it again"
    );
}

#[test]
fn the_mark_changes_with_a_new_run() {
    let root = tempfile::tempdir().expect("tempdir");
    write_run(root.path(), "20261117-100000", "md");
    let before = signature(root.path());
    assert_ne!(before, Signature::default());
    assert_eq!(signature(root.path()), before, "nothing changed");
    fs::write(out(root.path()).join("latest.json"), b"{\"runId\": \"x\"}").expect("latest");
    assert_ne!(signature(root.path()), before, "a run writes latest.json");
}

#[cfg(unix)]
#[test]
fn a_link_on_the_way_is_not_followed() {
    let root = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    write_run(elsewhere.path(), "20261117-100000", "md");
    // The run folder is a link into another tree.
    fs::create_dir_all(out(root.path())).expect("out");
    std::os::unix::fs::symlink(
        out(elsewhere.path()).join("20261117-100000"),
        out(root.path()).join("20261117-100000"),
    )
    .expect("link");
    assert!(read(root.path()).runs.is_empty());
    // `.rbrowser/out` itself a link.
    let other = tempfile::tempdir().expect("other");
    fs::create_dir_all(other.path().join(".rbrowser")).expect(".rbrowser");
    std::os::unix::fs::symlink(
        elsewhere.path().join(".rbrowser").join("out"),
        other.path().join(".rbrowser").join("out"),
    )
    .expect("link");
    assert!(read(other.path()).runs.is_empty());
}

#[test]
fn at_most_the_newest_runs_are_read() {
    let root = tempfile::tempdir().expect("tempdir");
    for minute in 0..3 {
        write_run(root.path(), &format!("20261117-1000{minute:02}"), "md");
    }
    let runs = read_at_most(root.path(), 2).runs;
    let ids: Vec<&str> = runs.iter().map(|run| run.run.as_str()).collect();
    assert_eq!(ids, ["20261117-100002", "20261117-100001"]);
}

// Review of the wave (RB-090-73): a read that is not whole - a summary that does not parse, as
// one caught while it is written - says so, so its mark is not kept; the other runs still count.
#[test]
fn a_half_written_summary_makes_the_read_partial() {
    let root = tempfile::tempdir().expect("tempdir");
    write_run(root.path(), "20261117-100000", "md");
    write_run(root.path(), "20261117-120000", "sm");
    let summary = out(root.path()).join("20261117-120000").join("run.json");
    fs::write(&summary, b"{\"runId\": \"20261117-1200").expect("half written");
    let partial = read(root.path());
    assert!(!partial.whole);
    let names: Vec<&str> = partial.runs.iter().map(|run| run.run.as_str()).collect();
    assert_eq!(names, ["20261117-100000"]);
    // A run folder without a summary yet is no reason to read again.
    let empty = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(out(empty.path()).join("20261117-130000")).expect("run folder");
    assert!(read(empty.path()).whole);
}

fn mark(seconds: u64) -> Signature {
    Signature {
        folder: Some(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds)),
        latest: None,
    }
}

// Review of the wave (RB-090-73): reads run at once and may end in any order; only the newest
// started read publishes, and the mark is kept only after a whole read that published.
#[test]
fn an_older_read_ending_last_does_not_publish() {
    let root = Path::new("/project");
    let mut gate = Gate::default();
    let older = gate.begin(root, &mark(1)).expect("a read is due");
    let newer = gate.begin(root, &mark(2)).expect("a read is due");
    assert!(
        gate.finish(newer, root, mark(2), true),
        "the newest publishes"
    );
    assert!(
        !gate.finish(older, root, mark(1), true),
        "the older is stale"
    );
    assert_eq!(gate.begin(root, &mark(2)), None, "the newest mark is kept");
    assert!(
        gate.begin(root, &mark(1)).is_some(),
        "the older mark is not"
    );
}

#[test]
fn an_older_read_ending_first_does_not_publish_either() {
    let root = Path::new("/project");
    let mut gate = Gate::default();
    let older = gate.begin(root, &mark(1)).expect("due");
    let newer = gate.begin(root, &mark(2)).expect("due");
    assert!(!gate.finish(older, root, mark(1), true));
    assert!(
        gate.begin(root, &mark(1)).is_some(),
        "nothing kept from a stale read"
    );
    // That begin started a newer read still: the one before it is stale now too.
    assert!(!gate.finish(newer, root, mark(2), true));
}

#[test]
fn a_partial_read_publishes_but_keeps_no_mark() {
    let root = Path::new("/project");
    let mut gate = Gate::default();
    let read = gate.begin(root, &mark(1)).expect("due");
    assert!(gate.finish(read, root, mark(1), false));
    assert!(
        gate.begin(root, &mark(1)).is_some(),
        "the same mark reads again"
    );
}

#[test]
fn a_mark_is_kept_per_project_and_cleared_with_the_trust() {
    let mut gate = Gate::default();
    let read = gate.begin(Path::new("/a"), &mark(1)).expect("due");
    assert!(gate.finish(read, Path::new("/a"), mark(1), true));
    assert!(
        gate.begin(Path::new("/b"), &mark(1)).is_some(),
        "another project"
    );
    let mut gate = Gate::default();
    let read = gate.begin(Path::new("/a"), &mark(1)).expect("due");
    gate.clear();
    assert!(
        !gate.finish(read, Path::new("/a"), mark(1), true),
        "a read under way when the trust went does not publish"
    );
    assert!(gate.begin(Path::new("/a"), &mark(1)).is_some());
}
