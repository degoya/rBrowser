use std::sync::Arc;

use super::{ListStep, Registry, ids_from_list, list_step};

#[test]
fn the_registry_hands_out_only_live_entries() {
    let registry = Registry::default();
    let first = Arc::new(1_u32);
    let second = Arc::new(2_u32);
    registry.add(&first);
    registry.add(&second);
    drop(first);
    let live: Vec<u32> = registry.live().iter().map(|entry| **entry).collect();
    assert_eq!(live, vec![2]);
    drop(second);
    assert!(registry.live().is_empty());
}

#[test]
fn the_id_list_is_read_up_to_its_count_and_its_buffer() {
    assert_eq!(ids_from_list(&[11, 12, 13, 0], 3), vec![11, 12, 13]);
    // Windows wrote fewer ids than it counts (the buffer was short): what is there.
    assert_eq!(ids_from_list(&[11, 12], 5), vec![11, 12]);
    // A process id is a 32-bit number; anything wider is no id.
    assert_eq!(ids_from_list(&[usize::MAX, 7], 2), vec![7]);
    assert_eq!(ids_from_list(&[], 0), Vec::<u32>::new());
}

/// Review 2: a list that did not fit is read again larger, a job that shrank meanwhile again at
/// the same size, and what fits is answered at the end - never nothing for a non-empty job.
#[test]
fn the_id_list_is_read_again_until_it_fits() {
    // It fit.
    assert_eq!(
        list_step(true, false, 2, 2, &[5, 6, 0], 4, 1, 0),
        ListStep::Done(vec![5, 6])
    );
    // 100 processes, room for 3: grow to hold them all, with room to spare.
    assert_eq!(
        list_step(false, true, 100, 3, &[5, 6, 7], 4, 1, 0),
        ListStep::Grow(117)
    );
    // The job shrank between the calls (it says 2, the buffer holds 3): read again.
    assert_eq!(
        list_step(false, true, 2, 3, &[5, 6, 7], 4, 1, 0),
        ListStep::Grow(4)
    );
    // Out of attempts or at the cap: what fit.
    assert_eq!(
        list_step(false, true, 100, 3, &[5, 6, 7], 4, 1, super::MAX_ATTEMPTS),
        ListStep::Done(vec![5, 6, 7])
    );
    assert_eq!(
        list_step(false, true, 100_000, 3, &[5, 6, 7], super::MAX_IDS, 1, 0),
        ListStep::Done(vec![5, 6, 7])
    );
    // Another error: what the buffer holds, if anything.
    assert_eq!(
        list_step(false, false, 0, 0, &[0, 0, 0], 4, 1, 0),
        ListStep::Done(vec![])
    );
}

#[cfg(not(windows))]
#[tokio::test]
async fn a_spawn_in_a_job_runs_without_one_outside_windows() {
    use crate::NoConsoleWindow;
    let mut command = tokio::process::Command::new("cargo");
    command
        .arg("--version")
        .no_console_window()
        .stdout(std::process::Stdio::null());
    let (mut child, job) = super::spawn_in_job(&mut command).expect("cargo starts");
    assert_eq!(
        job.expect_err("no job here").kind(),
        std::io::ErrorKind::Unsupported
    );
    assert_eq!(super::untied_spawns(), 0, "no job is no failure here");
    assert!(child.wait().await.expect("cargo ends").success());
}

#[cfg(not(windows))]
#[tokio::test]
async fn no_job_object_outside_windows() {
    use crate::NoConsoleWindow;
    let mut child = tokio::process::Command::new("cargo")
        .arg("--version")
        .no_console_window()
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("cargo starts");
    let error = super::ProcessJob::adopt(&child).expect_err("no job objects here");
    assert_eq!(error.kind(), std::io::ErrorKind::Unsupported);
    assert!(super::adopted_pids().is_empty());
    child.wait().await.expect("cargo ends");
}

/// RB-080-67: a process tree started in the job (suspended, then resumed - review 9) ends with
/// the job, grandchildren included.
#[cfg(windows)]
#[tokio::test]
async fn the_tree_ends_with_its_job() {
    use crate::NoConsoleWindow;
    // cmd.exe starts ping.exe: a tree of two.
    let mut command = tokio::process::Command::new("cmd.exe");
    command
        .args(["/C", "ping -n 60 127.0.0.1 > NUL"])
        .no_console_window();
    let (mut child, job) = super::spawn_in_job(&mut command).expect("cmd starts");
    let job = job.expect("the job takes cmd");
    let pid = child.id().expect("cmd runs");
    assert!(super::adopted_pids().contains(&pid));
    for _ in 0..100 {
        if job.active_processes() >= Some(2) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(job.active_processes() >= Some(2), "ping joined the job");
    assert!(job.pids().contains(&pid));
    drop(job);
    // Ending within the give-up deadline (ping runs 60 s) is the proof: a process the closing job
    // ends (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) exits with code 0, so the status says nothing.
    tokio::time::timeout(std::time::Duration::from_secs(10), child.wait())
        .await
        .expect("cmd ended with its job")
        .expect("a status");
}
