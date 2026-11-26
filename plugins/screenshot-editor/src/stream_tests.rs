use std::cell::RefCell;
use std::io::{BufRead, Read, Seek, SeekFrom, Write};

use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, PluginHost, codes};

use super::*;

const SHOT: &str = ".rbrowser/out/screenshots/long.png";

fn pattern(length: usize) -> Vec<u8> {
    (0..length).map(|i| (i % 253) as u8).collect()
}

#[test]
fn a_file_over_one_range_reads_whole_and_seeks() {
    let bytes = pattern(READ_BYTES * 2 + 17);
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(SHOT, &bytes);
    let shared: Shared<'_> = RefCell::new(&mut host);
    let mut file = RangeFile::open(&shared, SHOT).expect("open");
    assert_eq!(file.size(), bytes.len() as u64);
    let mut all = Vec::new();
    file.read_to_end(&mut all).expect("read");
    assert_eq!(all, bytes);
    // Back into the middle of the second range, then a few bytes.
    let at = READ_BYTES as u64 + 5;
    assert_eq!(file.seek(SeekFrom::Start(at)).expect("seek"), at);
    let mut few = [0u8; 4];
    file.read_exact(&mut few).expect("read");
    assert_eq!(few, bytes[READ_BYTES + 5..READ_BYTES + 9]);
    assert_eq!(
        file.seek(SeekFrom::End(0)).expect("end"),
        bytes.len() as u64
    );
    assert!(file.fill_buf().expect("at the end").is_empty());
    drop(file);
    // Three ranges for the whole file, one more after the seek back.
    assert_eq!(host.ranges.len(), 4);
}

#[test]
fn a_host_refusal_comes_back_as_itself() {
    let mut host = MemoryHost::new().with_file(SHOT, b"x");
    let shared: Shared<'_> = RefCell::new(&mut host);
    let refused = RangeFile::open(&shared, SHOT).err().expect("not granted");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));
    let wrapped = std::io::Error::other(refused.clone());
    assert_eq!(host_failure(&wrapped), Some(refused));
    assert_eq!(host_failure(&std::io::Error::other("other")), None);
}

#[test]
fn the_written_file_shows_only_once_committed() {
    let mut host = MemoryHost::new().grant(Capability::OutWrite);
    let part = pattern(PART_BYTES + 3);
    {
        let shared: Shared<'_> = RefCell::new(&mut host);
        let mut out = OutFile::begin(&shared, "edits/long.edited.png").expect("begun");
        out.write_all(&part).expect("written");
        out.write_all(b"tail").expect("written");
        assert_eq!(out.commit().expect("committed"), part.len() + 4);
    }
    let written = host.out.get("edits/long.edited.png").expect("there");
    assert_eq!(&written[..part.len()], &part[..]);
    assert_eq!(&written[part.len()..], b"tail");

    {
        let shared: Shared<'_> = RefCell::new(&mut host);
        let mut out = OutFile::begin(&shared, "edits/dropped.png").expect("begun");
        out.write_all(&part).expect("written");
        // Dropped without a commit: a failed render.
    }
    host.end_call();
    assert!(!host.out.contains_key("edits/dropped.png"));
    assert!(host.out_commit().is_err(), "nothing left to commit");
}
