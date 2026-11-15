//! The watch loop in-process over real pipes, against a temp dir and inotify (Linux).

use std::fs;
use std::io::{BufReader, PipeWriter};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread::JoinHandle;

use super::*;
use crate::protocol::{PROTOCOL_VERSION, encode, hello, new_nonce};
use crate::serve::read_line;
use crate::watch::{Change, ChangeKind};

const WAIT: Duration = Duration::from_secs(5);

struct Session {
    to_helper: PipeWriter,
    from_helper: Receiver<Message>,
    helper: JoinHandle<io::Result<Served>>,
}

impl Session {
    fn start(root: &Path) -> Self {
        let (in_r, in_w) = io::pipe().expect("a pipe");
        let (out_r, out_w) = io::pipe().expect("a pipe");
        let root = root.to_path_buf();
        let helper =
            std::thread::spawn(move || serve_watch(BufReader::new(in_r), out_w, "9.9.9", &root));
        let (tx, from_helper) = mpsc::channel();
        std::thread::spawn(move || {
            let mut out = BufReader::new(out_r);
            while let Ok(Line::Text(line)) = read_line(&mut out, MAX_LINE) {
                if tx.send(decode(&line).expect("a message")).is_err() {
                    return;
                }
            }
        });
        let mut session = Self {
            to_helper: in_w,
            from_helper,
            helper,
        };
        let nonce = new_nonce();
        session.send(&hello(&nonce));
        let Message::Welcome { protocol, .. } = session.next() else {
            panic!("a welcome");
        };
        assert_eq!(protocol, PROTOCOL_VERSION);
        session
    }

    fn send(&mut self, message: &Message) {
        self.to_helper
            .write_all(encode(message).as_bytes())
            .expect("written");
    }

    fn next(&self) -> Message {
        self.from_helper
            .recv_timeout(WAIT)
            .expect("a message in time")
    }

    fn quiet_for(&self, time: Duration) -> bool {
        self.from_helper.recv_timeout(time).is_err()
    }

    fn watch(&mut self, rules: WatchRules) -> u64 {
        self.send(&Message::Watch { rules });
        match self.next() {
            Message::Watching { folders } => folders,
            other => panic!("watching, not {other:?}"),
        }
    }

    fn changes(&self) -> Vec<Change> {
        match self.next() {
            Message::Changes { changes } => changes,
            other => panic!("changes, not {other:?}"),
        }
    }
}

fn rules(debounce_ms: u64, ignore: &[&str]) -> WatchRules {
    WatchRules {
        watch: Vec::new(),
        ignore: ignore.iter().map(|s| (*s).to_owned()).collect(),
        debounce_ms,
    }
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    for folder in ["src", "node_modules/lib", "generated"] {
        fs::create_dir_all(dir.path().join(folder)).expect("a folder");
    }
    dir
}

#[test]
fn a_burst_of_fifty_writes_is_one_batch() {
    let dir = project();
    let mut session = Session::start(dir.path());
    assert_eq!(session.watch(rules(150, &[])), 3, "root, src, generated");
    for i in 0..50 {
        fs::write(dir.path().join(format!("src/f{i}.js")), "x").expect("written");
    }
    let changes = session.changes();
    assert_eq!(changes.len(), 50, "{changes:?}");
    assert!(
        changes
            .iter()
            .all(|change| change.path.starts_with("src/f"))
    );
    assert!(session.quiet_for(Duration::from_millis(600)), "one batch");
}

#[test]
fn ignored_paths_report_nothing() {
    let dir = project();
    let mut session = Session::start(dir.path());
    session.watch(rules(50, &["generated/"]));
    fs::write(dir.path().join("node_modules/lib/index.js"), "x").expect("written");
    fs::write(dir.path().join("generated/out.css"), "x").expect("written");
    assert!(session.quiet_for(Duration::from_millis(400)));
    fs::write(dir.path().join("src/app.css"), "a{}").expect("written");
    let changes = session.changes();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "src/app.css");
}

#[test]
fn a_file_in_a_new_folder_is_seen() {
    let dir = project();
    let mut session = Session::start(dir.path());
    session.watch(rules(50, &[]));
    fs::create_dir(dir.path().join("src/new")).expect("a folder");
    let first = session.changes();
    assert_eq!(first, vec![Change::new("src/new", ChangeKind::Create)]);
    fs::write(dir.path().join("src/new/a.css"), "a{}").expect("written");
    let second = session.changes();
    assert!(second.iter().any(|change| change.path == "src/new/a.css"));
}

#[test]
fn new_rules_replace_the_old_ones() {
    let dir = project();
    let mut session = Session::start(dir.path());
    session.watch(rules(50, &[]));
    session.watch(rules(50, &["src/"]));
    fs::write(dir.path().join("src/app.css"), "a{}").expect("written");
    assert!(session.quiet_for(Duration::from_millis(400)));
}

#[test]
fn a_bad_rule_and_a_missing_folder_are_errors() {
    let dir = project();
    let mut session = Session::start(dir.path());
    session.send(&Message::Watch {
        rules: rules(50, &["src/[z-a]"]),
    });
    assert!(matches!(
        session.next(),
        Message::Error {
            code: ErrorCode::BadRule,
            ..
        }
    ));
    drop(session);

    let mut missing = Session::start(&dir.path().join("gone"));
    missing.send(&Message::Watch {
        rules: rules(50, &[]),
    });
    assert!(matches!(
        missing.next(),
        Message::Error {
            code: ErrorCode::WatchFailed,
            ..
        }
    ));
}

#[test]
fn ping_and_shutdown_work_while_watching() {
    let dir = project();
    let mut session = Session::start(dir.path());
    session.watch(rules(50, &[]));
    session.send(&Message::Ping);
    assert_eq!(session.next(), Message::Pong);
    session.send(&Message::Shutdown);
    let ended = session.helper.join().expect("the loop ended");
    assert_eq!(ended.expect("no pipe error"), Served::Shutdown);
}

#[test]
fn the_end_of_the_input_ends_the_watch() {
    let dir = project();
    let mut session = Session::start(dir.path());
    session.watch(rules(50, &[]));
    drop(session.to_helper);
    let ended = session.helper.join().expect("the loop ended");
    assert_eq!(ended.expect("no pipe error"), Served::Closed);
}
