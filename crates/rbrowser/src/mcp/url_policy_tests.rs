use std::path::Path;

use super::*;

fn url_of(path: &Path) -> String {
    format!("file://{}", path.display())
}

#[test]
fn http_and_https_pass_and_a_bare_host_is_completed() {
    for url in ["https://example.test/", "http://localhost:3000/a?b=1#c"] {
        assert_eq!(check(url, None).as_deref(), Ok(url));
    }
    assert_eq!(
        check("localhost:3000", None).as_deref(),
        Ok("http://localhost:3000")
    );
}

#[test]
fn every_other_scheme_is_refused_for_mcp() {
    for url in [
        "chrome://settings",
        "view-source:https://example.test/",
        "data:text/html,<h1>x</h1>",
        "about:blank",
        "CHROME://version",
    ] {
        let refused = check(url, Some(Path::new("/work/site"))).expect_err(url);
        assert!(refused.contains("only http and https"), "{url}: {refused}");
    }
    assert!(check("javascript:alert(1)", None).is_err());
}

#[test]
fn a_file_inside_the_project_passes() {
    let dir = tempfile::tempdir().expect("temp");
    let page = dir.path().join("dist/index.html");
    std::fs::create_dir_all(page.parent().expect("parent")).expect("dirs");
    std::fs::write(&page, "<h1>x</h1>").expect("page");
    let url = url_of(&page);
    assert_eq!(check(&url, Some(dir.path())).as_deref(), Ok(url.as_str()));
}

#[test]
fn a_file_outside_the_project_or_without_one_is_refused() {
    let project = tempfile::tempdir().expect("temp");
    let other = tempfile::tempdir().expect("temp");
    let secret = other.path().join("secret.html");
    std::fs::write(&secret, "x").expect("file");
    let inside = project.path().join("a.html");
    std::fs::write(&inside, "x").expect("file");

    let refused = check(&url_of(&secret), Some(project.path())).expect_err("outside");
    assert!(refused.contains("outside the project"), "{refused}");
    let refused = check(&url_of(&inside), None).expect_err("no project");
    assert!(refused.contains("no project"), "{refused}");
    let missing = check(
        &url_of(&project.path().join("gone.html")),
        Some(project.path()),
    )
    .expect_err("missing");
    assert!(missing.contains("cannot be read"), "{missing}");
}

#[test]
fn a_parent_step_a_foreign_host_or_a_broken_escape_is_refused_before_any_lookup() {
    let project = tempfile::tempdir().expect("temp");
    std::fs::write(project.path().join("a.html"), "x").expect("file");
    let climbing = format!("{}/sub/../a.html", url_of(project.path()));
    assert!(check(&climbing, Some(project.path())).is_err());
    assert!(check("file://server/share/a.html", Some(project.path())).is_err());
    assert!(check("file:///a%zz.html", Some(Path::new("/"))).is_err());
}

/// RB-090-30: a backslash is a slash to the browser, so `a\..\..\x.html` climbs out of the
/// project there; the policy reads it the same way, even where a file of that name exists.
#[cfg(unix)]
#[test]
fn a_backslash_parent_step_is_a_parent_step() {
    let project = tempfile::tempdir().expect("temp");
    let sub = project.path().join("sub");
    std::fs::create_dir_all(&sub).expect("dirs");
    std::fs::write(sub.join("a\\..\\..\\x.html"), "x").expect("a file named so");
    let url = format!("{}/a\\..\\..\\x.html", url_of(&sub));
    let refused = check(&url, Some(project.path())).expect_err("a parent step");
    assert!(refused.contains("outside the project"), "{refused}");
}

/// RB-090-30: a Windows path pasted after `file://` (backslashes, the drive in the host's
/// place, a drive letter in another case) is read as the browser reads it; a verbatim or UNC
/// form never passes.
#[cfg(windows)]
#[test]
fn a_windows_file_url_is_read_as_the_browser_reads_it() {
    let project = tempfile::tempdir().expect("temp");
    let page = project.path().join("a.html");
    std::fs::write(&page, "x").expect("file");
    let shown = page.display().to_string();
    let other_case = format!("{}{}", shown[..1].to_ascii_lowercase(), &shown[1..]);
    for url in [
        format!("file://{shown}"),
        format!("file:///{shown}"),
        format!("file:///{}", shown.replace('\\', "/")),
        format!("file://{other_case}"),
    ] {
        assert_eq!(
            check(&url, Some(project.path())).as_deref(),
            Ok(url.as_str()),
            "{url}"
        );
    }
    for url in [
        format!("file:///%5C%5C%3F%5C{shown}"),
        format!("file:////%3F/{shown}"),
        format!("file://127.0.0.1/{}", shown.replace(':', "$")),
        format!("file://{shown}\\..\\..\\a.html"),
    ] {
        assert!(check(&url, Some(project.path())).is_err(), "{url}");
    }
}

#[cfg(unix)]
#[test]
fn a_link_is_never_followed() {
    let project = tempfile::tempdir().expect("temp");
    let other = tempfile::tempdir().expect("temp");
    let secret = other.path().join("secret.html");
    std::fs::write(&secret, "x").expect("file");
    let page = project.path().join("page.html");
    std::fs::write(&page, "x").expect("file");

    let out = project.path().join("out.html");
    std::os::unix::fs::symlink(&secret, &out).expect("link");
    let refused = check(&url_of(&out), Some(project.path())).expect_err("a link out");
    assert!(refused.contains("link"), "{refused}");

    let within = project.path().join("within.html");
    std::os::unix::fs::symlink(&page, &within).expect("link");
    assert!(
        check(&url_of(&within), Some(project.path())).is_err(),
        "a link is refused even when it stays inside"
    );

    let folder = project.path().join("elsewhere");
    std::os::unix::fs::symlink(other.path(), &folder).expect("link");
    let through = check(&url_of(&folder.join("secret.html")), Some(project.path()))
        .expect_err("a folder link out");
    assert!(through.contains("outside the project"), "{through}");
}

#[test]
fn only_the_navigate_command_is_checked() {
    let refused = check_command(
        NAVIGATE,
        &serde_json::json!({ "url": "chrome://settings" }),
        None,
    )
    .expect_err("refused");
    assert!(
        matches!(refused, CommandError::InvalidArgs(_)),
        "{refused:?}"
    );
    assert!(
        check_command(
            NAVIGATE,
            &serde_json::json!({ "url": "https://a.test/" }),
            None
        )
        .is_ok()
    );
    assert!(check_command(NAVIGATE, &serde_json::json!({}), None).is_err());
    assert!(
        check_command(
            "reload.all",
            &serde_json::json!({ "url": "chrome://x" }),
            None
        )
        .is_ok()
    );
}

/// RB-094-10 (security review w28): `checks_run`'s `url` and `breakpoints_detect`'s `urls` are
/// held to the policy like `navigate`'s; the person's commands never pass this check.
#[test]
fn the_check_run_and_the_analyser_take_only_what_navigate_may_load() {
    let project = Some(Path::new("/project"));
    let refused = check_command(
        CHECKS_RUN,
        &serde_json::json!({ "url": "file:///etc/passwd" }),
        project,
    );
    assert!(
        matches!(refused, Err(CommandError::InvalidArgs(_))),
        "{refused:?}"
    );
    let refused = check_command(
        ANALYZE,
        &serde_json::json!({ "urls": ["https://a.test/", "file:///etc/passwd"] }),
        project,
    );
    assert!(
        matches!(refused, Err(CommandError::InvalidArgs(_))),
        "{refused:?}"
    );
    let refused = check_command(ANALYZE, &serde_json::json!({ "urls": [7] }), project);
    assert!(
        matches!(refused, Err(CommandError::InvalidArgs(_))),
        "{refused:?}"
    );

    for (id, args) in [
        (CHECKS_RUN, serde_json::json!({ "url": "https://a.test/" })),
        (CHECKS_RUN, serde_json::json!({ "plugins": ["seo"] })),
        (CHECKS_RUN, serde_json::Value::Null),
        (
            ANALYZE,
            serde_json::json!({ "urls": ["https://a.test/", "localhost:3000"] }),
        ),
        (ANALYZE, serde_json::json!({ "urls": [], "mode": "all" })),
        (ANALYZE, serde_json::Value::Null),
        (
            "view.reload",
            serde_json::json!({ "url": "file:///etc/passwd" }),
        ),
    ] {
        assert_eq!(check_command(id, &args, project), Ok(()), "{id} {args}");
    }
}

#[test]
fn the_command_ids_are_the_modules() {
    assert_eq!(CHECKS_RUN, crate::checks::RUN_ALL_COMMAND);
}
