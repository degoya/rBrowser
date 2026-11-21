//! [`crate::guest_host!`] expanded against a stand-in of what `wit_bindgen::generate!` writes for
//! `rbrowser:plugin` (the modules a component's `src/guest.rs` holds), so the guest half every
//! plugin shares is held to the contract natively, without a WebAssembly toolchain.

use std::cell::RefCell;

use crate::{
    Capability, Failure, FailureKind, Finding, HttpRequest, HttpResponse, PluginHost, Severity,
    ViewInfo,
};

thread_local! {
    static CALLS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Records one call of a stand-in binding.
fn call(line: String) {
    CALLS.with(|calls| calls.borrow_mut().push(line));
}

/// The calls the bindings saw since the last look.
fn calls() -> Vec<String> {
    CALLS.with(|calls| calls.borrow_mut().drain(..).collect())
}

/// The bindings of `rbrowser:plugin@0.1.0` as far as the macro reaches them: the same module
/// paths, records and function signatures wit-bindgen generates.
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

            /// What a stand-in answers when it fails.
            pub fn io_failure() -> Failure {
                Failure {
                    kind: FailureKind::Io,
                    code: Some("plugins.error.io".to_owned()),
                    params: vec![("path".to_owned(), "a.txt".to_owned())],
                }
            }
        }

        pub mod host {
            use crate::guest::tests::call;

            pub fn log(level: &str, message: &str) {
                call(format!("host.log {level} {message}"));
            }

            pub fn now_unix_seconds() -> u64 {
                call("host.now".to_owned());
                1_700_000_000
            }

            pub fn report_progress(done: u32, total: u32) {
                call(format!("host.progress {done}/{total}"));
            }
        }

        pub mod settings {
            use crate::guest::tests::call;

            pub fn get(key: &str) -> Option<String> {
                call(format!("settings.get {key}"));
                Some("true".to_owned())
            }
        }

        pub mod views {
            use crate::guest::tests::call;

            pub struct ViewInfo {
                pub id: u32,
                pub label: String,
                pub breakpoint: String,
                pub engine: String,
                pub width: u32,
                pub height: u32,
                pub dpr: f32,
                pub url: String,
                pub title: String,
            }

            pub fn list() -> Vec<ViewInfo> {
                call("views.list".to_owned());
                vec![ViewInfo {
                    id: 3,
                    label: "Phone".to_owned(),
                    breakpoint: "phone".to_owned(),
                    engine: "chromium".to_owned(),
                    width: 390,
                    height: 844,
                    dpr: 3.0,
                    url: "https://example.test/".to_owned(),
                    title: "Example".to_owned(),
                }]
            }

            pub fn active() -> Option<u32> {
                call("views.active".to_owned());
                Some(3)
            }
        }

        pub mod evaluate {
            use super::types::Failure;
            use crate::guest::tests::call;

            pub fn evaluate(view: u32, script: &str) -> Result<String, Failure> {
                call(format!("evaluate {view} {script}"));
                Ok("2".to_owned())
            }
        }

        pub mod cdp {
            use super::types::{Failure, io_failure};
            use crate::guest::tests::call;

            pub fn send(view: u32, method: &str, params: &str) -> Result<String, Failure> {
                call(format!("cdp.send {view} {method} {params}"));
                Err(io_failure())
            }
        }

        pub mod http {
            use super::types::Failure;
            use crate::guest::tests::call;

            pub struct HttpResponse {
                pub status: u16,
                pub headers: Vec<(String, String)>,
                pub body: Vec<u8>,
            }

            pub fn fetch(
                method: &str,
                url: &str,
                headers: &[(String, String)],
                body: &[u8],
            ) -> Result<HttpResponse, Failure> {
                call(format!(
                    "http.fetch {method} {url} {} {}",
                    headers.len(),
                    body.len()
                ));
                Ok(HttpResponse {
                    status: 204,
                    headers: vec![("etag".to_owned(), "x".to_owned())],
                    body: b"ok".to_vec(),
                })
            }
        }

        pub mod project {
            use super::types::Failure;
            use crate::guest::tests::call;

            pub fn read(path: &str) -> Result<Vec<u8>, Failure> {
                call(format!("project.read {path}"));
                Ok(b"{}".to_vec())
            }

            pub fn list(dir: &str) -> Result<Vec<String>, Failure> {
                call(format!("project.list {dir}"));
                Ok(vec!["a.json".to_owned()])
            }
        }

        pub mod project_write {
            use super::types::Failure;
            use crate::guest::tests::call;

            pub fn write(path: &str, bytes: &[u8], expected: Option<&[u8]>) -> Result<(), Failure> {
                call(format!(
                    "project-write.write {path} {} {}",
                    bytes.len(),
                    expected.map_or(0, <[u8]>::len)
                ));
                Ok(())
            }
        }

        pub mod out {
            use super::types::Failure;
            use crate::guest::tests::call;

            pub fn write(path: &str, bytes: &[u8]) -> Result<(), Failure> {
                call(format!("out.write {path} {}", bytes.len()));
                Ok(())
            }
        }

        pub mod findings {
            use super::types::{Failure, Finding};
            use crate::guest::tests::call;

            pub fn report(items: &[Finding]) -> Result<(), Failure> {
                let rules: Vec<&str> = items.iter().map(|item| item.rule.as_str()).collect();
                call(format!("findings.report {}", rules.join(",")));
                Ok(())
            }
        }

        pub mod sidecar {
            use super::types::Failure;

            pub fn call(method: &str, params: &str) -> Result<String, Failure> {
                crate::guest::tests::call(format!("sidecar.call {method} {params}"));
                Ok("{\"ok\":true}".to_owned())
            }
        }
    }
}

crate::guest_host! {
    /// Every interface granted.
    struct FullHost grants [
        settings, views, evaluate, cdp, http, project, project_write, out, findings, sidecar,
    ]
}

fn finding(rule: &str) -> Finding {
    Finding {
        rule: rule.to_owned(),
        severity: Severity::Error,
        message: format!("plugin.example.{rule}"),
        params: vec![("count".to_owned(), "2".to_owned())],
        view: Some(3),
        url: Some("https://example.test/".to_owned()),
        selector: Some("img".to_owned()),
        path: Some("html > body > img".to_owned()),
        data: Some("{}".to_owned()),
    }
}

#[test]
fn the_host_interface_is_always_reached() {
    let mut host = FullHost;
    host.log("info", "hello");
    assert_eq!(host.now_unix_seconds(), 1_700_000_000);
    host.report_progress(2, 5);
    assert_eq!(
        calls(),
        ["host.log info hello", "host.now", "host.progress 2/5"]
    );
}

#[test]
fn granted_interfaces_reach_their_bindings() {
    let mut host = FullHost;
    assert_eq!(host.setting("enabled").as_deref(), Some("true"));
    let views = host.views().expect("views");
    assert_eq!(
        views,
        [ViewInfo {
            id: 3,
            label: "Phone".to_owned(),
            breakpoint: "phone".to_owned(),
            engine: "chromium".to_owned(),
            width: 390,
            height: 844,
            dpr: 3.0,
            url: "https://example.test/".to_owned(),
            title: "Example".to_owned(),
        }]
    );
    assert_eq!(host.active_view().expect("active"), Some(3));
    assert_eq!(host.evaluate(3, "1 + 1").expect("evaluated"), "2");
    let request = HttpRequest {
        method: "GET".to_owned(),
        url: "https://example.test/".to_owned(),
        headers: vec![("accept".to_owned(), "*/*".to_owned())],
        body: Vec::new(),
    };
    assert_eq!(
        host.fetch(&request).expect("fetched"),
        HttpResponse {
            status: 204,
            headers: vec![("etag".to_owned(), "x".to_owned())],
            body: b"ok".to_vec(),
        }
    );
    assert_eq!(host.project_read(".rbrowser/a.json").expect("read"), b"{}");
    assert_eq!(host.project_list(".rbrowser").expect("listed"), ["a.json"]);
    host.project_write("todo.md", b"- [ ]", Some(b"-".as_slice()))
        .expect("written");
    host.out_write("report.html", b"<p>").expect("written");
    host.report_findings(&[finding("a"), finding("b")])
        .expect("reported");
    assert_eq!(
        host.sidecar_call("lighthouse.run", "{}").expect("called"),
        "{\"ok\":true}"
    );
    assert_eq!(
        calls(),
        [
            "settings.get enabled",
            "views.list",
            "views.active",
            "evaluate 3 1 + 1",
            "http.fetch GET https://example.test/ 1 0",
            "project.read .rbrowser/a.json",
            "project.list .rbrowser",
            "project-write.write todo.md 5 1",
            "out.write report.html 3",
            "findings.report a,b",
            "sidecar.call lighthouse.run {}",
        ]
    );
}

#[test]
fn a_binding_failure_reaches_the_plugin_as_the_native_failure() {
    let mut host = FullHost;
    let failure = host.cdp_send(3, "DOM.enable", "{}").expect_err("failed");
    assert_eq!(
        failure,
        Failure::coded(FailureKind::Io, "plugins.error.io").with_param("path", "a.txt")
    );
    assert_eq!(calls(), ["cdp.send 3 DOM.enable {}"]);
}

#[test]
fn failures_cross_the_boundary_both_ways_unchanged() {
    for kind in FailureKind::ALL {
        let native = Failure::coded(kind, "plugin.example.failed").with_param("n", 1);
        let wit = to_wit(native.clone());
        assert_eq!(wit.code.as_deref(), Some("plugin.example.failed"));
        assert_eq!(wit.params, [("n".to_owned(), "1".to_owned())]);
        assert_eq!(from_wit(wit), native, "{kind}");
    }
    let bare = Failure::new(FailureKind::Other);
    assert_eq!(from_wit(to_wit(bare.clone())), bare);
}

#[test]
fn findings_and_severities_convert_field_for_field() {
    assert_eq!(
        Severity::ALL.map(severity_to_wit),
        [
            rbrowser::plugin::types::Severity::Info,
            rbrowser::plugin::types::Severity::Warning,
            rbrowser::plugin::types::Severity::Error,
        ]
    );
    let native = finding("images.alt");
    assert_eq!(
        finding_to_wit(native.clone()),
        rbrowser::plugin::types::Finding {
            rule: native.rule,
            severity: rbrowser::plugin::types::Severity::Error,
            message: native.message,
            params: native.params,
            view: native.view,
            url: native.url,
            selector: native.selector,
            path: native.path,
            data: native.data,
        }
    );
}

/// Nothing granted: every capability's function answers `capability_denied` and never reaches
/// a binding, so the component imports none of them.
mod bare {
    use super::{Capability, HttpRequest, PluginHost, calls, rbrowser};

    crate::guest_host! {
        struct BareHost grants []
    }

    #[test]
    fn every_capability_is_refused_without_a_call() {
        let mut host = BareHost;
        assert_eq!(host.setting("enabled"), None);
        assert_eq!(host.views(), Err(Capability::Views.denied()));
        assert_eq!(host.active_view(), Err(Capability::Views.denied()));
        assert_eq!(host.evaluate(1, "1"), Err(Capability::Evaluate.denied()));
        assert_eq!(
            host.cdp_send(1, "DOM.enable", "{}"),
            Err(Capability::Cdp.denied())
        );
        assert_eq!(
            host.fetch(&HttpRequest::default()),
            Err(Capability::Http.denied())
        );
        assert_eq!(
            host.project_read("a"),
            Err(Capability::ProjectRead.denied())
        );
        assert_eq!(
            host.project_list("a"),
            Err(Capability::ProjectRead.denied())
        );
        assert_eq!(
            host.project_write("a", b"x", None),
            Err(Capability::ProjectWrite.denied())
        );
        assert_eq!(
            host.out_write("a", b"x"),
            Err(Capability::OutWrite.denied())
        );
        assert_eq!(
            host.report_findings(&[]),
            Err(Capability::Findings.denied())
        );
        assert_eq!(
            host.sidecar_call("a", "{}"),
            Err(Capability::Sidecar.denied())
        );
        assert!(calls().is_empty(), "no binding reached");
        // The host interface needs no grant.
        host.log("warn", "x");
        assert_eq!(calls(), ["host.log warn x"]);
    }
}

/// `project_read` alone: a plugin that reads one known file and never lists a folder keeps the
/// `list` import out.
mod read_only {
    use super::{Capability, PluginHost, calls, rbrowser};

    crate::guest_host! {
        struct ReadHost grants [project_read]
    }

    #[test]
    fn the_file_is_read_and_no_folder_is_listed() {
        let mut host = ReadHost;
        assert_eq!(host.project_read("a.json").expect("read"), b"{}");
        assert_eq!(
            host.project_list("."),
            Err(Capability::ProjectRead.denied())
        );
        assert_eq!(calls(), ["project.read a.json"]);
    }
}
