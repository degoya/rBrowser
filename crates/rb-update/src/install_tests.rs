//! A journal that reaches outside its installation is laid aside untouched and acted on by
//! nobody.

use std::fs;

use super::*;
use crate::install::fixture::{EXE, Fixture};

fn rejected(fixture: &Fixture) -> PathBuf {
    update_dir(&fixture.data).join(REJECTED_FILE)
}

#[test]
fn a_journal_pointing_out_of_its_folders_is_laid_aside_and_nothing_happens() {
    let cases: [fn(&mut Journal); 5] = [
        |journal| journal.plan.executable = "../evil.exe".to_owned(),
        |journal| journal.plan.data_dir = PathBuf::from("/elsewhere"),
        |journal| journal.entries = vec!["..".to_owned()],
        |journal| journal.entries = vec!["data".to_owned(), EXE.to_owned()],
        |journal| journal.replaced = vec!["notes.txt".to_owned()],
    ];
    for (index, change) in cases.into_iter().enumerate() {
        let mut fixture = Fixture::zip();
        let sound = fixture.journal.clone();
        change(&mut fixture.journal);
        let bytes = serde_json::to_vec_pretty(&fixture.journal).expect("encode");
        fs::write(Journal::path(&fixture.data), &bytes).expect("write");
        assert_eq!(
            Journal::read(&fixture.data).expect("read"),
            None,
            "case {index}"
        );
        assert!(!Journal::path(&fixture.data).exists(), "case {index}");
        assert_eq!(
            fs::read(rejected(&fixture)).expect("aside"),
            bytes,
            "case {index}"
        );
        let recovery =
            recover::recover_at_start(&fixture.data, &fixture.install.join(EXE), "1.0.0")
                .expect("recover");
        assert_eq!(recovery, recover::Recovery::Continue);
        fixture.journal = sound;
        fixture.assert_old();
    }
}

#[test]
fn a_sound_journal_reads_back_as_written() {
    let fixture = Fixture::zip();
    assert_eq!(fixture.stored(), fixture.journal);
    assert!(!rejected(&fixture).exists());
}

#[test]
fn preflight_refuses_a_folder_without_room() {
    let folder = tempfile::tempdir().expect("tempdir");
    assert!(preflight(folder.path(), 100, Some(200)).is_ok());
    assert!(preflight(folder.path(), 100, None).is_ok());
    let error = preflight(folder.path(), 100, Some(199)).expect_err("no room");
    assert_eq!(error.code, "update.not_enough_space");
    let missing = folder.path().join("missing");
    let error = preflight(&missing, 100, None).expect_err("not writable");
    assert_eq!(error.code, "update.install_dir_not_writable");
}

#[test]
fn a_plan_names_portable_installs_and_plain_versions_only() {
    let mut fixture = Fixture::zip();
    fixture.journal.plan.kind = crate::install_kind::InstallKind::Msi;
    assert_eq!(
        fixture.journal.plan.validate().map_err(|error| error.code),
        Err("update.plan_invalid")
    );
    let mut fixture = Fixture::zip();
    fixture.journal.plan.target_version = "../2.0.0".to_owned();
    assert!(fixture.journal.plan.validate().is_err());
}

/// The plan alone already refuses a program outside the program folder.
#[test]
fn a_plan_refuses_a_program_outside_the_program_folder() {
    for executable in ["../x", "a/../x", "/x", "", "a//x", "C:x", "a\\..\\x", ".x"] {
        let mut fixture = Fixture::zip();
        fixture.journal.plan.executable = executable.to_owned();
        assert_eq!(
            fixture.journal.plan.validate().map_err(|error| error.code),
            Err("update.plan_invalid"),
            "{executable:?}"
        );
    }
}

#[test]
fn every_phase_has_its_name_and_three_end_the_update() {
    let terminal: Vec<&str> = [
        Phase::Handed,
        Phase::Staged,
        Phase::Switching,
        Phase::Switched,
        Phase::Verified,
        Phase::RollingBack,
        Phase::RolledBack,
        Phase::Failed,
    ]
    .into_iter()
    .filter(|phase| phase.is_terminal())
    .map(Phase::as_str)
    .collect();
    assert_eq!(terminal, ["verified", "rolled_back", "failed"]);
}
