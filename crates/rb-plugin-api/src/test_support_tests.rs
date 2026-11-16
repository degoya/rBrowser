use super::MemoryHost;
use crate::{Capability, HttpRequest, PluginHost, codes};

fn get(url: &str) -> HttpRequest {
    HttpRequest {
        method: "GET".to_owned(),
        url: url.to_owned(),
        ..HttpRequest::default()
    }
}

#[test]
fn fetch_without_http_is_a_capability_failure() {
    let mut host = MemoryHost::new();
    let refused = host
        .fetch(&get("https://example.org/"))
        .expect_err("http is not granted");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));
    assert_eq!(refused.param("capability"), Some("http"));
    assert!(host.fetched.is_empty());

    let mut granted = MemoryHost::new().grant(Capability::Http);
    let answer = granted
        .fetch(&get("https://example.org/"))
        .expect("granted");
    assert_eq!(answer.status, 404);
    assert_eq!(granted.fetched.len(), 1);
}

#[test]
fn every_capability_is_checked() {
    let mut host = MemoryHost::new();
    let denied = |result: Result<(), crate::Failure>, capability: Capability| {
        let failure = result.expect_err("not granted");
        assert_eq!(failure.param("capability"), Some(capability.id()));
    };
    denied(host.views().map(drop), Capability::Views);
    denied(host.evaluate(1, "1").map(drop), Capability::Evaluate);
    denied(
        host.cdp_send(1, "DOM.getDocument", "{}").map(drop),
        Capability::Cdp,
    );
    denied(
        host.project_read(".rbrowser/config.json").map(drop),
        Capability::ProjectRead,
    );
    denied(host.out_write("a.txt", b"x"), Capability::OutWrite);
    denied(host.report_findings(&[]), Capability::Findings);
    denied(
        host.sidecar_call("lighthouse.run", "{}").map(drop),
        Capability::Sidecar,
    );
}

#[test]
fn writing_outside_the_out_folder_is_refused() {
    let mut host = MemoryHost::new().grant(Capability::OutWrite);
    for path in ["../escape.txt", "a/../../b.txt", "/abs.txt", "C:\\x.txt"] {
        let refused = host.out_write(path, b"x").expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
    assert!(host.out.is_empty());
    host.out_write("report/index.html", b"<p>").expect("inside");
    assert_eq!(
        host.out.get("report/index.html").map(Vec::as_slice),
        Some(&b"<p>"[..])
    );
}

#[test]
fn project_read_covers_the_config_dir_and_the_globs_only() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/config.json", b"{}")
        .with_file(".rbrowser/config.local.json", b"{\"token\":1}")
        .with_file("src/main.css", b"a{}")
        .with_file("secret.env", b"KEY=1");
    host.readable.insert("src/main.css".to_owned());
    assert_eq!(
        host.project_read(".rbrowser/config.json").expect("config"),
        b"{}"
    );
    assert_eq!(host.project_read("src/main.css").expect("glob"), b"a{}");
    for path in [".rbrowser/config.local.json", "secret.env"] {
        let refused = host.project_read(path).expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_NOT_GRANTED), "{path}");
    }
    let refused = host
        .project_read(".rbrowser/../secret.env")
        .expect_err("climbs");
    assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE));
    assert_eq!(
        host.project_list(".rbrowser").expect("list"),
        vec!["config.json".to_owned()]
    );
}

#[test]
fn cdp_reaches_the_listed_domains_only() {
    let mut host = MemoryHost::new().grant(Capability::Cdp);
    host.cdp_domains = vec!["CSS".to_owned()];
    assert!(host.cdp_send(1, "CSS.getStyleSheetText", "{}").is_ok());
    let refused = host
        .cdp_send(1, "Runtime.evaluate", "{}")
        .expect_err("not listed");
    assert_eq!(refused.code(), Some(codes::CDP_DOMAIN_DENIED));
    assert_eq!(host.cdp_sent.len(), 1);
}

#[test]
fn project_write_replaces_a_granted_file_only_when_it_is_what_was_read() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectWrite)
        .with_file("TODO.todo", b"old");
    for path in ["TODO.todo", "new.todo", "unread.todo"] {
        host.writable.insert(path.to_owned());
    }
    for path in ["TODO.todo", "new.todo"] {
        host.readable.insert(path.to_owned());
    }

    let untrusted = host
        .project_write("TODO.todo", b"new", Some(b"old"))
        .expect_err("untrusted");
    assert_eq!(untrusted.code(), Some(codes::UNTRUSTED_WORKSPACE));

    host.trusted = true;
    let conflict = host
        .project_write("TODO.todo", b"new", Some(b"changed"))
        .expect_err("changed on disk");
    assert_eq!(conflict.code(), Some(codes::WRITE_CONFLICT));
    assert_eq!(host.project["TODO.todo"], b"old");

    host.project_write("TODO.todo", b"new", Some(b"old"))
        .expect("written");
    assert_eq!(host.project["TODO.todo"], b"new");

    let exists = host
        .project_write("TODO.todo", b"x", None)
        .expect_err("not new");
    assert_eq!(exists.code(), Some(codes::WRITE_CONFLICT));
    host.project_write("new.todo", b"x", None).expect("created");

    // A path no read glob covers is never written, as in the app (code review RB-110-13).
    for path in [
        "other.todo",
        "unread.todo",
        ".git/x.todo",
        ".rbrowser/config.json",
    ] {
        let refused = host.project_write(path, b"x", None).expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_NOT_WRITABLE), "{path}");
    }
    let outside = host
        .project_write("../x.todo", b"x", None)
        .expect_err("climbs");
    assert_eq!(outside.code(), Some(codes::PATH_OUTSIDE));
}

#[test]
fn project_write_without_the_capability_is_refused() {
    let mut host = MemoryHost::new();
    host.trusted = true;
    let refused = host
        .project_write("a.todo", b"x", None)
        .expect_err("not granted");
    assert_eq!(refused.param("capability"), Some("project-write"));
}

#[test]
fn an_empty_folder_lists_the_root() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file("TODO.todo", b"")
        .with_file("docs/plan.todo", b"");
    host.readable.insert("TODO.todo".to_owned());
    host.readable.insert("docs/plan.todo".to_owned());
    assert_eq!(
        host.project_list("").expect("root"),
        vec!["TODO.todo".to_owned(), "docs".to_owned()]
    );
}
