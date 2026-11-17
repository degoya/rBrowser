use rb_plugin_types::{CheckTarget, FINDINGS_VERSION, FindingsFile, FindingsSummary};

use super::{findings_path, run_id, valid_run_id, write_run};

fn file(run_id: &str) -> FindingsFile {
    FindingsFile {
        version: FINDINGS_VERSION,
        run_id: run_id.to_owned(),
        started_at: 1,
        target: CheckTarget::All,
        url: None,
        plugins: vec!["seo".to_owned()],
        views: Vec::new(),
        findings: Vec::new(),
        summary: FindingsSummary::default(),
        failures: Vec::new(),
        skipped: Vec::new(),
    }
}

#[test]
fn run_ids_sort_by_time_and_nothing_else_is_one() {
    assert_eq!(run_id(42), "run-0000000000042");
    assert!(run_id(9) < run_id(10));
    assert!(valid_run_id(&run_id(1_700_000_000_000)));
    for bad in [
        "run-",
        "run-12a",
        "../run-1",
        "run-1/..",
        "other",
        "run-12345678901234567890",
    ] {
        assert!(!valid_run_id(bad), "{bad}");
    }
}

#[tokio::test]
async fn a_run_is_written_as_versioned_json_and_only_the_last_runs_kept() {
    let project = tempfile::tempdir().expect("temp");
    for at in 1..=4 {
        write_run(project.path(), &file(&run_id(at)), 2)
            .await
            .expect("written");
    }
    let checks = project.path().join(".rbrowser/out/checks");
    let mut kept: Vec<String> = std::fs::read_dir(&checks)
        .expect("checks folder")
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .collect();
    kept.sort();
    assert_eq!(kept, vec![run_id(3), run_id(4)]);
    let text =
        std::fs::read_to_string(findings_path(project.path(), &run_id(4))).expect("findings.json");
    let back: FindingsFile = serde_json::from_str(&text).expect("parses");
    assert_eq!(back, file(&run_id(4)));
    assert!(text.contains("\"version\": 1"));
}

#[tokio::test]
async fn a_bad_run_id_writes_nothing() {
    let project = tempfile::tempdir().expect("temp");
    assert!(
        write_run(project.path(), &file("../escape"), 2)
            .await
            .is_err()
    );
    assert!(!project.path().join(".rbrowser").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn a_linked_out_folder_is_refused() {
    let project = tempfile::tempdir().expect("temp");
    let elsewhere = tempfile::tempdir().expect("temp");
    std::fs::create_dir_all(project.path().join(".rbrowser")).expect("config dir");
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join(".rbrowser/out"))
        .expect("link");
    assert!(
        write_run(project.path(), &file(&run_id(1)), 2)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(elsewhere.path()).expect("dir").count(), 0);
}
