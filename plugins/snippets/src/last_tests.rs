use super::{LAST_PATH, Last, read, remember};
use crate::catalog::Source;
use crate::fake_host::FakeHost;

#[test]
fn the_last_run_is_written_and_read_back() {
    let mut host = FakeHost::with_views(Vec::new());
    assert_eq!(read(&mut host), None);
    let last = Last {
        source: Source::Project,
        name: "Layout grid".to_owned(),
        views: vec![2, 3],
    };
    remember(&mut host, &last);
    let written = String::from_utf8(host.files[LAST_PATH].clone()).expect("UTF-8");
    assert!(!written.contains("code"), "never the code: {written}");
    assert_eq!(read(&mut host), Some(last));
}

#[test]
fn a_file_that_does_not_read_is_no_last_run() {
    let mut host = FakeHost::with_views(Vec::new());
    for text in [
        "not json",
        r#"{"source":"web","name":"a","views":[1]}"#,
        r#"{"source":"user","name":"","views":[1]}"#,
        r#"{"source":"user","name":"a","views":[]}"#,
        r#"{"source":"user","name":"a","views":[-1]}"#,
    ] {
        host.file(LAST_PATH, text);
        assert_eq!(read(&mut host), None, "{text}");
    }
}

#[test]
fn a_write_that_fails_is_logged() {
    let mut host = FakeHost::with_views(Vec::new());
    host.no_project = true;
    remember(
        &mut host,
        &Last {
            source: Source::User,
            name: "a".to_owned(),
            views: vec![1],
        },
    );
    assert!(host.logs.iter().any(|(level, _)| level == "warn"));
}
