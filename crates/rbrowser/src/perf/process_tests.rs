use super::*;

fn entry(pid: u32, parent: u32, mb: u64) -> ProcEntry {
    ProcEntry {
        pid,
        parent,
        private_bytes: Some(mb * 1024 * 1024),
        started: None,
        name: None,
    }
}

fn named(pid: u32, parent: u32, name: &str) -> ProcEntry {
    ProcEntry {
        name: Some(name.to_owned()),
        ..entry(pid, parent, 10)
    }
}

fn started(pid: u32, parent: u32, mb: u64, at: u64) -> ProcEntry {
    ProcEntry {
        started: Some(at),
        ..entry(pid, parent, mb)
    }
}

#[test]
fn the_total_is_the_root_and_every_descendant() {
    let entries = [
        entry(10, 1, 100),
        // The renderers and the GPU process.
        entry(11, 10, 200),
        entry(12, 10, 300),
        // A grandchild (a utility process of a helper).
        entry(13, 12, 50),
        // Somebody else's.
        entry(20, 1, 9000),
        entry(21, 20, 9000),
    ];
    let sample = tree_total(10, &entries).expect("the root exists");
    assert_eq!(sample.processes, 4);
    assert_eq!(sample.private_bytes, 650 * 1024 * 1024);
}

#[test]
fn a_process_whose_memory_could_not_be_read_counts_without_bytes() {
    let mut entries = vec![entry(10, 1, 100), entry(11, 10, 200)];
    entries.push(ProcEntry {
        pid: 12,
        parent: 10,
        private_bytes: None,
        started: None,
        name: None,
    });
    let sample = tree_total(10, &entries).expect("the root exists");
    assert_eq!(sample.processes, 3);
    assert_eq!(sample.private_bytes, 300 * 1024 * 1024);
}

#[test]
fn a_parent_cycle_from_reused_pids_ends() {
    let entries = [entry(10, 11, 100), entry(11, 10, 200)];
    let sample = tree_total(10, &entries).expect("the root exists");
    assert_eq!(sample.processes, 2);
}

#[test]
fn no_root_no_sample() {
    assert_eq!(tree_total(10, &[entry(11, 1, 5)]), None);
}

#[test]
fn the_parent_is_read_after_the_command_name_even_with_parentheses_in_it() {
    let stat = "4242 (rb (x) helper) S 4200 4242 4200 0 -1 4194560 1 0 0 0";
    assert_eq!(parse_stat_parent(stat), Some(4200));
    assert_eq!(parse_stat_parent("garbage"), None);
}

#[test]
fn the_private_memory_is_rss_anon_in_kib() {
    let status = "Name:\trbrowser\nVmRSS:\t  9000 kB\nRssAnon:\t  2048 kB\nRssFile:\t 100 kB\n";
    assert_eq!(parse_rss_anon(status), Some(2048 * 1024));
    assert_eq!(parse_rss_anon("Name:\tx\n"), None);
}

#[cfg(target_os = "linux")]
#[test]
fn the_test_process_itself_is_sampled_on_linux() {
    let census = sample().expect("/proc is readable");
    let sample = census.sample;
    assert!(sample.processes >= 1);
    assert_eq!(census.names.values().sum::<u32>(), sample.processes);
    assert!(sample.private_bytes > 0);
}

/// F9 (RB-020-09): a process whose parent id was reused by one of ours - it started before that
/// "parent" - is somebody else's, and so is everything below it.
#[test]
fn a_process_older_than_its_parent_is_not_its_child() {
    let entries = [
        started(10, 1, 100, 50),
        started(11, 10, 200, 60),
        // Its parent 11 died long ago; the id went to our renderer.
        started(12, 11, 9000, 40),
        started(13, 12, 9000, 70),
        // A start time that could not be read prunes nothing.
        entry(14, 11, 25),
    ];
    let sample = tree_total(10, &entries).expect("the root exists");
    assert_eq!(sample.processes, 3);
    assert_eq!(sample.private_bytes, 325 * 1024 * 1024);
}

/// F9: the descendants are found in one walk over a map of children; a cycle of reused ids
/// ends, somebody else's stay out.
#[test]
fn the_descendants_are_found_in_one_walk() {
    let links = [
        (10, 1),
        (11, 10),
        (12, 10),
        (13, 12),
        (20, 1),
        (21, 20),
        (30, 31),
        (31, 30),
        (0, 0),
    ];
    assert_eq!(descendants(10, &links), BTreeSet::from([10, 11, 12, 13]));
    assert_eq!(descendants(30, &links), BTreeSet::from([30, 31]));
    assert_eq!(descendants(0, &links), BTreeSet::from([0]));
}

/// RB-080-67: on Windows a browser child whose main process has exited keeps the dead parent's
/// id - it is in no tree any more and was never counted. A process of one of our job objects
/// counts, with everything below it, wherever its parent id points.
#[test]
fn an_adopted_orphan_counts_with_its_children() {
    let entries = [
        entry(10, 1, 100),
        entry(11, 10, 200),
        // The sidecar; its Lighthouse browser 29 has exited.
        entry(12, 10, 50),
        entry(30, 29, 20),
        entry(31, 29, 20),
        entry(32, 30, 20),
        // Somebody else's orphan.
        entry(40, 39, 9000),
    ];
    assert_eq!(
        tree_total(10, &entries).expect("the root exists").processes,
        3
    );
    let adopted = BTreeSet::from([12, 30, 31, 32, 99]);
    let census = tree_census(10, &adopted, &entries).expect("the root exists");
    assert_eq!(census.sample.processes, 6);
    assert_eq!(census.sample.private_bytes, 410 * 1024 * 1024);
}

/// RB-080-67: the log names what the processes are.
#[test]
fn the_census_names_the_most_frequent_images() {
    let entries = [
        named(10, 1, "rbrowser.exe"),
        named(11, 10, "rbrowser.exe"),
        named(12, 10, "node.exe"),
        named(13, 12, "chrome.exe"),
        named(14, 13, "chrome.exe"),
        named(15, 13, "chrome.exe"),
        entry(16, 13, 10),
    ];
    let census = tree_census(10, &BTreeSet::new(), &entries).expect("the root exists");
    assert_eq!(census.top(2), "chrome.exe=3, rbrowser.exe=2");
    assert_eq!(
        census.top(9),
        "chrome.exe=3, rbrowser.exe=2, ?=1, node.exe=1"
    );
}

#[test]
fn the_image_name_is_read_up_to_its_end() {
    let mut raw = [0_u16; 8];
    for (slot, unit) in raw.iter_mut().zip("node.exe".encode_utf16()) {
        *slot = unit;
    }
    assert_eq!(image_name(&raw).as_deref(), Some("node.exe"));
    let mut short = [0_u16; 8];
    short[..3].copy_from_slice(&[0x61, 0x62, 0x63]);
    assert_eq!(image_name(&short).as_deref(), Some("abc"));
    assert_eq!(image_name(&[0_u16; 4]), None);
}

/// Review 6: a process's details (memory, start, name) are read only for rBrowser's own tree
/// and its job objects', never for every process of the system, and not at all for the links.
#[test]
fn details_are_read_only_for_our_processes() {
    let list = || {
        vec![
            (10, 1, ()),
            (11, 10, ()),
            (20, 1, ()),
            (21, 20, ()),
            (30, 29, ()),
            (31, 30, ()),
        ]
    };
    let read = std::cell::RefCell::new(Vec::new());
    let reader = |pid: u32, (): &()| {
        read.borrow_mut().push(pid);
        Details {
            private_bytes: Some(1),
            started: None,
            name: Some("x".to_owned()),
        }
    };
    let entries = entries_from(list(), Some((10, &BTreeSet::from([30]))), reader);
    assert_eq!(entries.len(), 6);
    let mut seen = read.borrow().clone();
    seen.sort_unstable();
    assert_eq!(seen, vec![10, 11, 30, 31]);
    let stranger = entries
        .iter()
        .find(|entry| entry.pid == 21)
        .expect("listed");
    assert_eq!(
        (stranger.private_bytes, stranger.name.as_deref()),
        (None, None)
    );
    read.borrow_mut().clear();
    assert_eq!(entries_from(list(), None, reader).len(), 6);
    assert!(read.borrow().is_empty());
}
