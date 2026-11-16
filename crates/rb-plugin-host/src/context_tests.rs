use std::sync::Arc;

use rb_plugin_types::{Capability, HttpRequest, codes};

use super::PluginContext;
use crate::manifest::Grants;
use crate::services::NoServices;
use crate::test_support::ScriptedServices;

fn grants(table: &str) -> Grants {
    Grants::parse(&toml::from_str(table).expect("toml")).expect("grants")
}

fn context(table: &str, root: &std::path::Path) -> PluginContext {
    PluginContext::new(
        "sample",
        "1.0.0",
        grants(table),
        Arc::new(ScriptedServices::default()),
    )
    .with_project(Some(root.to_path_buf()))
}

#[tokio::test]
async fn writing_outside_the_plugins_out_folder_is_refused() {
    let project = tempfile::tempdir().expect("tempdir");
    let plugin = context("out-write = true", project.path());
    for path in [
        "../escape.txt",
        "../other-plugin/x.txt",
        "a/../../../config.json",
        "/etc/escape.txt",
        "C:\\escape.txt",
        "..\\escape.txt",
    ] {
        let refused = plugin.out_write(path, b"x").await.expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE), "{path}");
    }
    assert!(!project.path().join(".rbrowser/out/escape.txt").exists());
    assert!(!project.path().join(".rbrowser/out/other-plugin").exists());

    plugin
        .out_write("report/index.html", b"<p>ok</p>")
        .await
        .expect("inside its folder");
    let written = project
        .path()
        .join(".rbrowser/out/sample/report/index.html");
    assert_eq!(std::fs::read(written).expect("written"), b"<p>ok</p>");
}

#[cfg(unix)]
#[tokio::test]
async fn a_linked_out_folder_is_never_written_through() {
    let project = tempfile::tempdir().expect("tempdir");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(project.path().join(".rbrowser")).expect("config dir");
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join(".rbrowser/out"))
        .expect("link");
    let plugin = context("out-write = true", project.path());
    let refused = plugin
        .out_write("report.txt", b"x")
        .await
        .expect_err("linked folder");
    assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE));
    assert_eq!(std::fs::read_dir(elsewhere.path()).expect("dir").count(), 0);
}

#[tokio::test]
async fn project_read_covers_the_config_dir_and_the_globs_but_never_the_local_config() {
    let project = tempfile::tempdir().expect("tempdir");
    let root = project.path();
    std::fs::create_dir_all(root.join(".rbrowser")).expect("dir");
    std::fs::create_dir_all(root.join("src/css")).expect("dir");
    std::fs::write(root.join(".rbrowser/config.json"), b"{}").expect("write");
    std::fs::write(root.join(".rbrowser/config.local.json"), b"{\"t\":1}").expect("write");
    std::fs::write(root.join("src/css/main.css"), b"a{}").expect("write");
    std::fs::write(root.join("src/app.js"), b"x").expect("write");
    let plugin = context("project-read = { globs = [\"src/**/*.css\"] }", root);
    assert_eq!(
        plugin
            .project_read(".rbrowser/config.json")
            .await
            .expect("config"),
        b"{}"
    );
    assert_eq!(
        plugin.project_read("src/css/main.css").await.expect("glob"),
        b"a{}"
    );
    for path in [".rbrowser/config.local.json", "src/app.js"] {
        let refused = plugin.project_read(path).await.expect_err(path);
        assert_eq!(refused.code(), Some(codes::PATH_NOT_GRANTED), "{path}");
    }
    let refused = plugin.project_read("../outside").await.expect_err("climbs");
    assert_eq!(refused.code(), Some(codes::PATH_OUTSIDE));
    assert_eq!(
        plugin.project_list(".rbrowser").await.expect("list"),
        vec!["config.json".to_owned()]
    );
    assert_eq!(
        plugin.project_list("src").await.expect("list"),
        vec!["css/".to_owned()]
    );
}

#[tokio::test]
async fn fetch_without_the_http_capability_is_a_capability_failure() {
    let project = tempfile::tempdir().expect("tempdir");
    let plugin = context("out-write = true", project.path());
    let refused = plugin
        .fetch(HttpRequest {
            method: "GET".to_owned(),
            url: "https://example.org/".to_owned(),
            ..HttpRequest::default()
        })
        .await
        .expect_err("http not granted");
    assert_eq!(refused.code(), Some(codes::CAPABILITY_DENIED));
    assert_eq!(refused.param("capability"), Some(Capability::Http.id()));
}

#[tokio::test]
async fn fetch_with_http_reaches_the_service_and_checks_the_url() {
    let project = tempfile::tempdir().expect("tempdir");
    let plugin = context("http = true", project.path());
    let answer = plugin
        .fetch(HttpRequest {
            method: "GET".to_owned(),
            url: "http://localhost:3000/".to_owned(),
            ..HttpRequest::default()
        })
        .await
        .expect("granted");
    assert_eq!(answer.status, 200);
    let refused = plugin
        .fetch(HttpRequest {
            method: "GET".to_owned(),
            url: "file:///etc/passwd".to_owned(),
            ..HttpRequest::default()
        })
        .await
        .expect_err("not http");
    assert_eq!(refused.code(), Some(codes::HTTP_URL_DENIED));
}

#[tokio::test]
async fn cdp_reaches_the_listed_domains_and_an_unwired_service_says_so() {
    let project = tempfile::tempdir().expect("tempdir");
    let plugin = context("cdp = { domains = [\"CSS\"] }", project.path());
    assert_eq!(
        plugin
            .cdp_send(1, "CSS.enable", "{}")
            .await
            .expect("listed"),
        "{}"
    );
    let refused = plugin
        .cdp_send(1, "Runtime.evaluate", "{}")
        .await
        .expect_err("not listed");
    assert_eq!(refused.code(), Some(codes::CDP_DOMAIN_DENIED));

    let unwired = PluginContext::new(
        "sample",
        "1.0.0",
        grants("evaluate = true"),
        Arc::new(NoServices),
    );
    let refused = unwired.evaluate(1, "1").await.expect_err("no backend");
    assert_eq!(refused.code(), Some(codes::NOT_AVAILABLE));
}

#[tokio::test]
async fn without_a_project_the_file_functions_are_not_available() {
    let plugin = PluginContext::new(
        "sample",
        "1.0.0",
        grants("out-write = true"),
        Arc::new(NoServices),
    );
    let refused = plugin
        .out_write("a.txt", b"x")
        .await
        .expect_err("no project");
    assert_eq!(refused.code(), Some(codes::NOT_AVAILABLE));
}

#[tokio::test]
async fn the_local_config_stays_hidden_under_another_case_or_a_short_name() {
    // Review finding M3 of Charge 6: on Windows `.RBROWSER/config.local.json` and
    // `.rbrowser/CONFIG~1.JSO` open the real file, and `**/*.json` matched them.
    let project = tempfile::tempdir().expect("tempdir");
    let folder = project.path().join(".RBROWSER");
    std::fs::create_dir_all(&folder).expect("dir");
    std::fs::write(folder.join("config.local.json"), b"{\"token\":1}").expect("write");
    std::fs::write(folder.join("config.json"), b"{}").expect("write");
    let plugin = context("project-read = { globs = [\"**/*.json\"] }", project.path());
    for path in [
        ".RBROWSER/config.local.json",
        ".RBROWSER/CONFIG.LOCAL.JSON",
        ".rbrowser/CONFIG~1.JSO",
    ] {
        let refused = plugin.project_read(path).await.expect_err(path);
        assert!(
            matches!(
                refused.code(),
                Some(codes::PATH_NOT_GRANTED | codes::PATH_OUTSIDE)
            ),
            "{path}: {refused:?}"
        );
    }
    let names = plugin.project_list(".RBROWSER").await.expect("listed");
    assert_eq!(names, vec!["config.json".to_owned()]);
}
