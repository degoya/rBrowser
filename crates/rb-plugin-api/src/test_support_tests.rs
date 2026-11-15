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
