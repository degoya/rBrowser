use super::*;

const RUN: &str = "20261112-100000";
const OLD: &str = "20261101-090000";

fn project() -> tempfile::TempDir {
    let project = tempfile::tempdir().expect("tempdir");
    for run in [RUN, OLD] {
        let folder = project.path().join(".rbrowser/out/lighthouse").join(run);
        std::fs::create_dir_all(&folder).expect("folders");
        std::fs::write(folder.join("mobile.html"), "<html>report</html>").expect("report");
    }
    project
}

fn open(rel: &str) -> Ask {
    Ask {
        open: Some(rel.to_owned()),
        ..Ask::default()
    }
}

#[test]
fn an_untrusted_project_opens_and_removes_nothing() {
    let project = project();
    let ask = Ask {
        open: Some(format!("{RUN}/mobile.html")),
        window: Placement::Single,
        remove: vec![OLD.to_owned()],
    };
    let outcome = apply(
        Some(project.path().to_path_buf()),
        false,
        "lighthouse",
        &ask,
    );
    assert_eq!(outcome.open, None);
    assert_eq!(outcome.open_refused, Some(Refusal::Untrusted));
    assert_eq!(
        outcome.not_removed,
        vec![(OLD.to_owned(), Refusal::Untrusted)]
    );
    assert!(
        project
            .path()
            .join(".rbrowser/out/lighthouse")
            .join(OLD)
            .is_dir()
    );
    let outcome = apply(None, true, "lighthouse", &ask);
    assert_eq!(outcome.open_refused, Some(Refusal::NoProject));
}

#[test]
fn a_report_opens_at_an_address_that_resolves_to_it() {
    let project = project();
    let outcome = apply(
        Some(project.path().to_path_buf()),
        true,
        "lighthouse",
        &open(&format!("{RUN}/mobile.html")),
    );
    let opened = outcome.open.expect("opened");
    assert!(tokens::is_report_url(&opened.url));
    assert!(
        !opened.url.contains(RUN),
        "the address carries no path: {}",
        opened.url
    );
    let root = Some(project.path().to_path_buf());
    let file = resolve(&opened.url, root.clone(), true).expect("resolves");
    assert!(file.ends_with("mobile.html"));
    revoke(&opened.url);
    assert_eq!(resolve(&opened.url, root, true), Err(Refusal::NotFound));
}

#[test]
fn a_report_outside_the_folder_gets_no_address() {
    let project = project();
    let outcome = apply(
        Some(project.path().to_path_buf()),
        true,
        "lighthouse",
        &open("../../config.json.html"),
    );
    assert_eq!(outcome.open, None);
    assert_eq!(outcome.open_refused, Some(Refusal::Path));
}

#[cfg(unix)]
#[test]
fn a_report_swapped_for_a_link_after_the_open_is_not_served() {
    let project = project();
    let elsewhere = tempfile::tempdir().expect("tempdir");
    std::fs::write(elsewhere.path().join("secret.html"), "secret").expect("file");
    let outcome = apply(
        Some(project.path().to_path_buf()),
        true,
        "lighthouse",
        &open(&format!("{RUN}/mobile.html")),
    );
    let url = outcome.open.expect("opened").url;
    let report = project
        .path()
        .join(".rbrowser/out/lighthouse")
        .join(RUN)
        .join("mobile.html");
    std::fs::remove_file(&report).expect("removed");
    std::os::unix::fs::symlink(elsewhere.path().join("secret.html"), &report).expect("link");
    assert_eq!(
        resolve(&url, Some(project.path().to_path_buf()), true),
        Err(Refusal::Link)
    );
}

#[test]
fn a_run_is_removed_before_its_report_could_open() {
    let project = project();
    let ask = Ask {
        open: Some(format!("{OLD}/mobile.html")),
        window: Placement::PerReport,
        remove: vec![
            OLD.to_owned(),
            "../..".to_owned(),
            "20991231-000000".to_owned(),
        ],
    };
    let outcome = apply(Some(project.path().to_path_buf()), true, "lighthouse", &ask);
    assert_eq!(outcome.removed, vec![OLD.to_owned()]);
    assert_eq!(
        outcome.not_removed,
        vec![
            ("../..".to_owned(), Refusal::Path),
            ("20991231-000000".to_owned(), Refusal::NotFound),
        ]
    );
    assert!(
        !project
            .path()
            .join(".rbrowser/out/lighthouse")
            .join(OLD)
            .exists()
    );
    assert!(
        project
            .path()
            .join(".rbrowser/out/lighthouse")
            .join(RUN)
            .is_dir()
    );
    assert!(project.path().exists());
    assert_eq!(outcome.open_refused, Some(Refusal::NotFound));
}

#[test]
fn the_outcome_goes_into_the_answer() {
    let ask = Ask {
        open: Some("a/b.html".to_owned()),
        window: Placement::Single,
        remove: vec!["x".to_owned()],
    };
    let outcome = Outcome {
        open: None,
        open_refused: Some(Refusal::NotFound),
        removed: Vec::new(),
        not_removed: vec![("x".to_owned(), Refusal::Link)],
    };
    let mut answer = json!({ "runs": [] });
    outcome.merge_into(&ask, &mut answer);
    assert_eq!(
        answer,
        json!({
            "runs": [],
            "window": { "opened": false, "reason": "report_not_found" },
            "removed": [],
            "notRemoved": [{ "run": "x", "reason": "report_link" }],
        })
    );
}

#[test]
fn a_token_dies_when_trust_is_revoked_or_the_project_changes() {
    // Security review L1: the admission is checked again on every load.
    let other = project();
    let project = project();
    let root = Some(project.path().to_path_buf());
    let url = apply(
        root.clone(),
        true,
        "lighthouse",
        &open(&format!("{RUN}/mobile.html")),
    )
    .open
    .expect("opened")
    .url;
    assert!(resolve(&url, root.clone(), true).is_ok());
    assert_eq!(resolve(&url, root.clone(), false), Err(Refusal::Untrusted));
    assert_eq!(resolve(&url, None, true), Err(Refusal::NoProject));
    assert_eq!(
        resolve(&url, Some(other.path().to_path_buf()), true),
        Err(Refusal::NotFound)
    );
    assert!(
        resolve(&url, root, true).is_ok(),
        "trusted again, the same project"
    );
}
