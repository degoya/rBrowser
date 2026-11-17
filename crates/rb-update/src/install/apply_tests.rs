//! The updater's run on a fixture installation: what it changes and what it leaves, without a
//! window and without starting a program.

use std::fs;

use super::*;
use crate::install::fixture::Fixture;

fn no_wait() -> Duration {
    Duration::ZERO
}

fn plenty(_: &Path) -> Option<u64> {
    Some(u64::MAX)
}

fn run_fixture(fixture: &mut Fixture) -> i32 {
    fixture.journal.write().expect("journal");
    run_with(&Journal::path(&fixture.data), &plenty, no_wait()).expect("run")
}

/// "Install when quitting": the switch is made, the new version waits for the next start.
#[test]
fn an_update_left_to_the_next_start_switches_and_starts_nothing() {
    for mut fixture in [Fixture::zip(), Fixture::portable()] {
        assert_eq!(run_fixture(&mut fixture), 0);
        assert_eq!(fixture.stored().phase, Phase::Switched);
        fixture.assert_new();
    }
}

/// While a window still runs, nothing changes and nothing starts.
#[test]
fn a_window_that_does_not_end_leaves_everything_as_it_was() {
    let mut fixture = Fixture::zip();
    fixture.journal.plan.start_new = true;
    let _window = InstanceLock::share(&fixture.data)
        .expect("lock")
        .expect("free");
    assert_eq!(run_fixture(&mut fixture), 1);
    let stored = fixture.stored();
    assert_eq!(stored.phase, Phase::Failed);
    assert_eq!(stored.reason.as_deref(), Some("update.app_did_not_end"));
    fixture.assert_old();
}

/// An artifact exchanged after the check is never unpacked: the hash is checked again.
#[test]
fn an_artifact_swapped_after_the_check_is_not_unpacked() {
    let mut fixture = Fixture::zip();
    let mut bytes = fs::read(&fixture.journal.plan.artifact).expect("artifact");
    bytes[10] ^= 0xff;
    fs::write(&fixture.journal.plan.artifact, bytes).expect("swap");
    assert_eq!(run_fixture(&mut fixture), 1);
    assert_eq!(
        fixture.stored().reason.as_deref(),
        Some("update.digest_mismatch")
    );
    fixture.assert_old();
    assert!(!fixture.journal.staged_dir().exists());
}

#[test]
fn a_program_folder_without_room_is_refused_before_anything_moves() {
    let mut fixture = Fixture::zip();
    fixture.journal.write().expect("journal");
    let tight = |_: &Path| Some(1);
    let code = run_with(&Journal::path(&fixture.data), &tight, no_wait()).expect("run");
    assert_eq!(code, 1);
    assert_eq!(
        fixture.stored().reason.as_deref(),
        Some("update.not_enough_space")
    );
    fixture.assert_old();
}

#[test]
fn only_a_handed_over_update_is_applied() {
    let mut fixture = Fixture::zip();
    fixture.journal.phase = Phase::Verified;
    fixture.journal.write().expect("journal");
    assert!(run_with(&Journal::path(&fixture.data), &plenty, no_wait()).is_err());
    fixture.assert_old();
}
