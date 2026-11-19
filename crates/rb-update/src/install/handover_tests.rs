use std::fs;

use super::*;
use crate::install::fixture::{EXE, Fixture, NEW, OLD};
use crate::manifest::tests::artifact;

fn running(fixture: &Fixture) -> Running {
    Running {
        executable: fixture.install.join(EXE),
        version: OLD.to_owned(),
        kind: InstallKind::Portable,
        data_dir: fixture.data.clone(),
        args: vec!["--project".to_owned(), "C:\\site".to_owned()],
    }
}

#[test]
fn the_plan_names_the_running_programs_folder_and_the_verified_file() {
    let fixture = Fixture::zip();
    let downloaded = download_dir(&fixture.data).join("rbrowser-1.1.0-windows-x64.zip");
    let offered = artifact("windows", "x86_64", "zip");
    let plan = plan(&running(&fixture), NEW, &offered, &downloaded, true).expect("plan");
    assert_eq!(plan.install_dir, fixture.install);
    assert_eq!(plan.executable, EXE);
    assert_eq!(plan.artifact, downloaded);
    assert_eq!(plan.sha256, offered.sha256);
    assert_eq!(plan.size, offered.size);
    assert_eq!(plan.args, ["--project", "C:\\site"]);
    assert!(plan.start_new);
    assert_eq!(plan.confirm_timeout_secs, DEFAULT_CONFIRM_TIMEOUT_SECS);
}

#[test]
fn a_package_managers_installation_gets_no_plan() {
    let fixture = Fixture::zip();
    let scoop = Running {
        kind: InstallKind::Scoop,
        ..running(&fixture)
    };
    let error = plan(
        &scoop,
        NEW,
        &artifact("windows", "x86_64", "zip"),
        &fixture.journal.plan.artifact,
        true,
    )
    .expect_err("scoop updates itself");
    assert_eq!(error.code, "update.plan_invalid");
}

/// A package without the updater beside the program: refused before a journal is written, so
/// nothing waits for a start that never comes.
#[test]
fn without_the_shipped_updater_no_journal_is_written() {
    let fixture = Fixture::zip();
    fs::remove_file(Journal::path(&fixture.data)).expect("no journal yet");
    let error = hand_over(fixture.journal.plan.clone()).expect_err("no updater");
    assert_eq!(error.code, "update.updater_missing");
    assert!(Journal::read(&fixture.data).expect("read").is_none());
}

#[test]
fn a_failed_or_taken_back_version_is_not_installed_by_itself_again() {
    let fixture = Fixture::zip();
    assert!(!failed_before(&fixture.data, NEW), "only handed");
    let mut journal = fixture.stored();
    journal
        .end(Phase::RolledBack, "update.not_confirmed", "test")
        .expect("end");
    assert!(failed_before(&fixture.data, NEW));
    assert!(!failed_before(&fixture.data, "1.2.0"), "another version");
}

/// RB-090-14: a superseded download goes; the offer's, the journal's and a download in progress
/// stay.
#[test]
fn superseded_downloads_are_pruned_and_the_live_ones_kept() {
    let fixture = Fixture::zip();
    let folder = download_dir(&fixture.data);
    fs::create_dir_all(&folder).expect("folder");
    let old = folder.join("rbrowser-1.0.5-windows-x64.zip");
    let offered = folder.join("rbrowser-1.2.0-windows-x64.zip");
    let partial = folder.join("rbrowser-1.3.0-windows-x64.zip.partial");
    for path in [&old, &offered, &partial] {
        fs::write(path, b"zip").expect("write");
    }
    let journal_artifact = folder.join("rbrowser-1.1.0-windows-x64.zip");
    fs::write(&journal_artifact, b"zip").expect("write");
    let mut journal = fixture.stored();
    journal.plan.artifact = journal_artifact.clone();
    journal.write().expect("journal");
    assert_eq!(prune_downloads(&fixture.data, &[offered.as_path()]), 1);
    assert!(!old.exists());
    assert!(offered.exists());
    assert!(partial.exists());
    assert!(journal_artifact.exists());
}
