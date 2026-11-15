//! What the next start does after a stop at each phase: it ends in the whole old or the whole new
//! version, never in between - a crash mid-switch included, recovered to version A.

use std::fs;

use super::*;
use crate::install::UpdaterLock;
use crate::install::fixture::{EXE, Fixture, NEW, OLD};
use crate::install::portable::tests::partial_switch;

fn exe(fixture: &Fixture) -> PathBuf {
    fixture.journal.executable()
}

fn stored_phase(fixture: &Fixture) -> (Phase, Option<String>) {
    let stored = fixture.stored();
    (stored.phase, stored.reason)
}

#[test]
fn a_stop_before_the_switch_leaves_the_old_version_and_records_the_interruption() {
    for staged in [false, true] {
        let mut fixture = Fixture::zip();
        if staged {
            portable::stage(&mut fixture.journal).expect("stage");
        }
        let recovery = recover_at_start(&fixture.data, &exe(&fixture), OLD).expect("recover");
        assert_eq!(recovery, Recovery::Continue);
        assert_eq!(
            stored_phase(&fixture),
            (Phase::Failed, Some("update.interrupted".to_owned()))
        );
        fixture.assert_old();
    }
}

/// The acceptance of RB-090-02: a crash in the middle of the switch, after any rename, and the
/// next start's `recover` puts version A back whole - in the portable mode too.
#[test]
fn a_crash_in_the_middle_of_the_switch_is_recovered_to_the_old_version() {
    let renames = {
        let mut fixture = Fixture::zip();
        portable::stage(&mut fixture.journal).expect("stage");
        fixture.journal.entries.len() + fixture.journal.replaced.len()
    };
    for portable_mode in [false, true] {
        for stop_after in 1..renames {
            let mut fixture = if portable_mode {
                Fixture::portable()
            } else {
                Fixture::zip()
            };
            portable::stage(&mut fixture.journal).expect("stage");
            fixture
                .journal
                .advance(Phase::Switching)
                .expect("switching");
            partial_switch(&fixture.journal, stop_after);
            // Whichever program file is in place now starts; it may be either version.
            let running = if fixture.read(EXE).as_deref() == Some("new program") {
                NEW
            } else {
                OLD
            };
            let recovery =
                recover_at_start(&fixture.data, &exe(&fixture), running).expect("recover");
            let expected = if running == OLD {
                Recovery::Continue
            } else {
                Recovery::Restart(exe(&fixture))
            };
            assert_eq!(recovery, expected, "stop after {stop_after}");
            assert_eq!(
                stored_phase(&fixture),
                (Phase::RolledBack, Some("update.interrupted".to_owned()))
            );
            fixture.assert_old();
        }
    }
}

/// Switched and never proven: the first start of the new version may prove it; a second
/// unproven one takes it back.
#[test]
fn a_second_unproven_start_of_the_new_version_takes_it_back() {
    let mut fixture = Fixture::zip();
    portable::stage(&mut fixture.journal).expect("stage");
    portable::switch(&mut fixture.journal).expect("switch");
    let first = recover_at_start(&fixture.data, &exe(&fixture), NEW).expect("first");
    assert_eq!(first, Recovery::Continue);
    assert_eq!(fixture.stored().start_attempts, 1);
    fixture.assert_new();
    let second = recover_at_start(&fixture.data, &exe(&fixture), NEW).expect("second");
    assert_eq!(second, Recovery::Restart(exe(&fixture)));
    assert_eq!(
        stored_phase(&fixture),
        (Phase::RolledBack, Some("update.not_confirmed".to_owned()))
    );
    fixture.assert_old();
}

#[test]
fn the_first_start_proves_the_update_and_a_later_one_cleans_up_once() {
    for mut fixture in [Fixture::zip(), Fixture::portable()] {
        portable::stage(&mut fixture.journal).expect("stage");
        portable::switch(&mut fixture.journal).expect("switch");
        assert_eq!(
            recover_at_start(&fixture.data, &exe(&fixture), NEW).expect("recover"),
            Recovery::Continue
        );
        assert!(!confirm_started(&fixture.data, &exe(&fixture), OLD).expect("other version"));
        assert!(confirm_started(&fixture.data, &exe(&fixture), NEW).expect("confirm"));
        assert_eq!(fixture.stored().phase, Phase::Verified);
        recover_at_start(&fixture.data, &exe(&fixture), NEW).expect("clean");
        assert!(fixture.stored().cleaned);
        assert!(
            !fixture.journal.previous_dir().exists(),
            "the old version is gone"
        );
        assert_eq!(fixture.read(EXE).as_deref(), Some("new program"));
        fixture.assert_untouched();
    }
}

/// The fixture of the job: version A to B with a real journal, and back again.
#[test]
fn an_update_from_a_to_b_and_its_rollback_with_a_real_journal() {
    let mut fixture = Fixture::zip();
    portable::stage(&mut fixture.journal).expect("stage");
    portable::switch(&mut fixture.journal).expect("switch");
    fixture.assert_new();
    let on_disk: serde_json::Value =
        serde_json::from_slice(&fs::read(Journal::path(&fixture.data)).expect("journal"))
            .expect("json");
    assert_eq!(on_disk["phase"], "switched");
    take_back(&mut fixture.journal, "update.not_confirmed", "test").expect("take back");
    fixture.assert_old();
    assert_eq!(
        stored_phase(&fixture),
        (Phase::RolledBack, Some("update.not_confirmed".to_owned()))
    );
}

#[test]
fn another_program_sharing_the_data_changes_nothing() {
    let fixture = Fixture::zip();
    let elsewhere = fixture.install.with_file_name("source-build").join(EXE);
    assert_eq!(
        recover_at_start(&fixture.data, &elsewhere, OLD).expect("recover"),
        Recovery::Continue
    );
    assert_eq!(fixture.stored().phase, Phase::Handed);
}

/// A build of the same version started from elsewhere (a source build sharing the data) does
/// not prove the switched update: only the program in the journal's folder may (charge-4
/// review, finding 6).
#[test]
fn another_program_of_the_new_version_does_not_prove_the_update() {
    let mut fixture = Fixture::zip();
    portable::stage(&mut fixture.journal).expect("stage");
    portable::switch(&mut fixture.journal).expect("switch");
    let elsewhere = fixture.install.with_file_name("source-build").join(EXE);
    assert!(!confirm_started(&fixture.data, &elsewhere, NEW).expect("elsewhere"));
    assert_eq!(fixture.stored().phase, Phase::Switched);
    assert!(confirm_started(&fixture.data, &exe(&fixture), NEW).expect("confirm"));
}

#[test]
fn a_running_updater_is_left_alone() {
    let fixture = Fixture::zip();
    let _updater = UpdaterLock::acquire(&fixture.data)
        .expect("lock")
        .expect("free");
    assert_eq!(
        recover_at_start(&fixture.data, &exe(&fixture), OLD).expect("recover"),
        Recovery::Continue
    );
    assert_eq!(fixture.stored().phase, Phase::Handed);
}
