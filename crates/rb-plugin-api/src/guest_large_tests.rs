//! [`crate::guest_host!`] with the large-file interfaces of RB-099-24, `project-range` and
//! `out-stream`, against a stand-in of their generated bindings: granted they reach the
//! bindings, not granted they keep the trait's refusal and reach nothing.

use std::cell::RefCell;

use crate::{Capability, PluginHost};

thread_local! {
    static CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn call(line: String) {
    CALLS.with(|calls| calls.borrow_mut().push(line));
}

fn calls() -> Vec<String> {
    CALLS.with(|calls| calls.borrow_mut().drain(..).collect())
}

/// What wit-bindgen generates for the interfaces the macro reaches here.
#[allow(
    dead_code,
    unreachable_pub,
    reason = "a stand-in of generated bindings: public like them, some fields never read"
)]
mod rbrowser {
    pub mod plugin {
        pub mod types {
            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum FailureKind {
                NotFound,
                AccessDenied,
                NotSupported,
                BadData,
                Cancelled,
                Io,
                Limit,
                Other,
            }

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub struct Failure {
                pub kind: FailureKind,
                pub code: Option<String>,
                pub params: Vec<(String, String)>,
            }

            #[derive(Clone, Copy, Debug, PartialEq, Eq)]
            pub enum Severity {
                Info,
                Warning,
                Error,
            }

            #[derive(Clone, Debug, PartialEq, Eq)]
            pub struct Finding {
                pub rule: String,
                pub severity: Severity,
                pub message: String,
                pub params: Vec<(String, String)>,
                pub view: Option<u32>,
                pub url: Option<String>,
                pub selector: Option<String>,
                pub path: Option<String>,
                pub data: Option<String>,
            }
        }

        pub mod host {
            pub fn log(_level: &str, _message: &str) {}

            pub fn now_unix_seconds() -> u64 {
                0
            }

            pub fn report_progress(_done: u32, _total: u32) {}
        }

        pub mod project_range {
            use super::types::Failure;
            use crate::guest::large_tests::call;

            pub fn read(path: &str, offset: u64, length: u32) -> Result<(Vec<u8>, u64), Failure> {
                call(format!("project-range.read {path} {offset} {length}"));
                Ok((b"PNG".to_vec(), 9_000_000))
            }
        }

        pub mod out_stream {
            use super::types::{Failure, FailureKind};
            use crate::guest::large_tests::call;

            pub fn begin(path: &str) -> Result<(), Failure> {
                call(format!("out-stream.begin {path}"));
                Ok(())
            }

            pub fn append(bytes: &[u8]) -> Result<(), Failure> {
                call(format!("out-stream.append {}", bytes.len()));
                Ok(())
            }

            pub fn commit() -> Result<(), Failure> {
                call("out-stream.commit".to_owned());
                Err(Failure {
                    kind: FailureKind::Limit,
                    code: Some("plugins.error.too_large".to_owned()),
                    params: Vec::new(),
                })
            }
        }
    }
}

crate::guest_host! {
    /// The two large-file interfaces granted.
    struct LargeHost grants [project_range, out_stream]
}

#[test]
fn granted_large_file_interfaces_reach_their_bindings() {
    let mut host = LargeHost;
    assert_eq!(
        host.project_read_range("a.png", 8, 3).expect("read"),
        (b"PNG".to_vec(), 9_000_000)
    );
    host.out_begin("a.png").expect("begun");
    host.out_append(b"12345").expect("appended");
    let failure = host.out_commit().expect_err("the stand-in refuses");
    assert_eq!(failure.code.as_deref(), Some("plugins.error.too_large"));
    assert_eq!(
        calls(),
        [
            "project-range.read a.png 8 3",
            "out-stream.begin a.png",
            "out-stream.append 5",
            "out-stream.commit",
        ]
    );
    // Not among the grants: the plain read and write stay refused.
    assert_eq!(
        host.project_read("a.png"),
        Err(Capability::ProjectRead.denied())
    );
    assert_eq!(
        host.out_write("a.png", b"x"),
        Err(Capability::OutWrite.denied())
    );
}

/// Not granted: the trait's refusals, so a component never imports either interface.
mod bare {
    use super::{Capability, PluginHost, calls, rbrowser};

    crate::guest_host! {
        struct BareHost grants []
    }

    #[test]
    fn the_large_file_interfaces_are_refused_without_a_call() {
        let mut host = BareHost;
        assert_eq!(
            host.project_read_range("a.png", 0, 1),
            Err(Capability::ProjectRead.denied())
        );
        assert_eq!(host.out_begin("a.png"), Err(Capability::OutWrite.denied()));
        assert_eq!(host.out_append(b"x"), Err(Capability::OutWrite.denied()));
        assert_eq!(host.out_commit(), Err(Capability::OutWrite.denied()));
        assert!(calls().is_empty(), "no binding reached");
    }
}
