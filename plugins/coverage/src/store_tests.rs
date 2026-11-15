use super::{LOAD_STALE_SECONDS, Session, hash, read_source, session, write_source};
use crate::fake::FakeHost;
use crate::model::Mode;
use crate::resolve::Cache;

#[test]
fn a_load_run_cut_off_long_ago_no_longer_blocks() {
    let running = Session {
        running: Some(Mode::Load),
        started: 100,
        ..Session::default()
    };
    assert_eq!(running.active(100 + 60), Some(Mode::Load));
    assert_eq!(running.active(100 + LOAD_STALE_SECONDS + 1), None);
    let interaction = Session {
        running: Some(Mode::Interaction),
        ..running
    };
    assert_eq!(interaction.active(u64::MAX), Some(Mode::Interaction));
}

#[test]
fn no_stored_session_is_idle() {
    let mut host = FakeHost::new();
    assert_eq!(session(&mut host), Session::default());
}

#[test]
fn a_source_is_written_once_and_read_back_by_its_key_only() {
    let mut host = FakeHost::new();
    let mut cache = Cache::default();
    let key = write_source(&mut host, "a { }", &mut cache).expect("written");
    assert_eq!(key, hash("a { }"));
    assert_eq!(
        write_source(&mut host, "a { }", &mut cache),
        Some(key.clone())
    );
    assert_eq!(host.written.len(), 1);
    assert_eq!(read_source(&mut host, &key).as_deref(), Some("a { }"));
    assert_eq!(read_source(&mut host, "../../config.json"), None);
}
