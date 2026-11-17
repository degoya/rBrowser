use std::collections::BTreeMap;

use rb_config::{Breakpoint, EngineKind, Environment, ProjectConfig};
use serde_json::json;

use super::*;

fn breakpoint(id: &str, width: u32, engine: EngineKind) -> Breakpoint {
    serde_json::from_value(json!({
        "id": id,
        "width": width,
        "height": 800,
        "engine": serde_json::to_value(engine).expect("engine"),
    }))
    .expect("a breakpoint")
}

fn config() -> ProjectConfig {
    let mut config = ProjectConfig {
        start_url: Some("https://www.example.test/".to_owned()),
        ..ProjectConfig::default()
    };
    config.breakpoints = vec![
        breakpoint("mobile", 375, EngineKind::Chromium),
        breakpoint("desktop", 1280, EngineKind::Chromium),
        breakpoint("fox", 1024, EngineKind::Firefox),
    ];
    config.environments = vec![Environment {
        id: "local".to_owned(),
        url: "http://127.0.0.1:8080/".to_owned(),
        color: None,
        reload_interval_sec: None,
        asset_origins: Vec::new(),
    }];
    config
}

fn args(value: serde_json::Value) -> CaptureArgs {
    CaptureArgs::parse(&value).expect("args")
}

#[test]
fn a_bare_origin_is_a_site_and_a_path_a_sitemap() {
    let root = tempfile::tempdir().expect("temp");
    let config = config();
    let start = source(&args(json!(null)), &config, root.path(), 1024).expect("start url");
    assert_eq!(start.source, json!({ "site": "https://www.example.test" }));
    let sitemap = source(
        &args(json!({ "url": "https://www.example.test/sitemap_index.xml" })),
        &config,
        root.path(),
        1024,
    )
    .expect("sitemap");
    assert_eq!(
        sitemap.source,
        json!({ "sitemap": "https://www.example.test/sitemap_index.xml" })
    );
    let none = ProjectConfig::default();
    assert_eq!(
        source(&args(json!(null)), &none, root.path(), 1024),
        Err(PlanError::NoSource)
    );
}

#[test]
fn a_project_file_is_read_inside_the_project_only() {
    let root = tempfile::tempdir().expect("temp");
    std::fs::write(
        root.path().join("pages.txt"),
        "https://www.example.test/a\n",
    )
    .expect("file");
    std::fs::write(root.path().join("notes.md"), "x").expect("file");
    let config = config();
    let file = source(
        &args(json!({ "url": "pages.txt" })),
        &config,
        root.path(),
        1024,
    )
    .expect("reads");
    let encoded = base64::engine::general_purpose::STANDARD.encode("https://www.example.test/a\n");
    assert_eq!(file.source, json!({ "file": encoded }));
    assert_eq!(file.from.as_deref(), Some("https://www.example.test"));
    for refused in ["../pages.txt", "notes.md", "/etc/hosts.txt", "missing.xml"] {
        let error = source(&args(json!({ "url": refused })), &config, root.path(), 1024)
            .expect_err(refused);
        assert_eq!(error.code(), "sitemap.error.bad_file", "{refused}");
    }
    let small = source(
        &args(json!({ "url": "pages.txt" })),
        &config,
        root.path(),
        4,
    );
    assert!(matches!(small, Err(PlanError::BadFile(_))));
}

#[test]
fn the_environment_is_the_rewrites_target() {
    let root = tempfile::tempdir().expect("temp");
    let config = config();
    let call = args(json!({ "env": "local" }));
    let site = source(&call, &config, root.path(), 1024).expect("site");
    assert_eq!(
        rewrite(&call, &config, &site).expect("rewrite"),
        Some(json!({ "to": "http://127.0.0.1:8080" }))
    );
    assert_eq!(rewrite(&args(json!(null)), &config, &site), Ok(None));
    assert_eq!(
        rewrite(&args(json!({ "env": "prod" })), &config, &site),
        Err(PlanError::UnknownEnv("prod".to_owned()))
    );
}

#[test]
fn every_chromium_breakpoint_by_default_others_skipped() {
    let plan = breakpoints(&args(json!(null)), &config(), &[], true).expect("plan");
    let names: Vec<&str> = plan.run.iter().map(|bp| bp.name.as_str()).collect();
    assert_eq!(names, ["mobile", "desktop"]);
    assert_eq!(plan.skipped, ["fox"]);
    let mobile = &plan.specs["mobile"];
    assert_eq!(mobile.emulation.width, 375);
    assert_eq!(mobile.session, "default");
    assert_eq!(plan.run[1].width, 1280);
}

#[test]
fn named_breakpoints_and_session_are_checked() {
    let config = config();
    let one = breakpoints(
        &args(json!({ "breakpoints": ["desktop"] })),
        &config,
        &[],
        true,
    )
    .expect("plan");
    assert_eq!(one.run.len(), 1);
    assert_eq!(
        breakpoints(
            &args(json!({ "breakpoints": ["watch"] })),
            &config,
            &[],
            true
        ),
        Err(PlanError::UnknownBreakpoint("watch".to_owned()))
    );
    assert_eq!(
        breakpoints(&args(json!({ "session": "ghost" })), &config, &[], true),
        Err(PlanError::UnknownSession("ghost".to_owned()))
    );
    assert_eq!(
        breakpoints(&args(json!({ "breakpoints": ["fox"] })), &config, &[], true),
        Err(PlanError::NoBreakpoints)
    );
}

#[test]
fn project_headers_wait_for_trust() {
    let mut config = config();
    config.emulation.headers = BTreeMap::from([("X-Preview".to_owned(), "1".to_owned())]);
    let untrusted = breakpoints(&args(json!(null)), &config, &[], false).expect("plan");
    assert!(untrusted.specs["mobile"].emulation.headers.is_empty());
    let trusted = breakpoints(&args(json!(null)), &config, &[], true).expect("plan");
    assert_eq!(trusted.specs["mobile"].emulation.headers.len(), 1);
}
