//! The job object on Windows (RB-099-17): a child in the job ends when the job is closed or
//! terminated, a suspended child runs once resumed, an ended child joins no job. Runs natively
//! on Windows only. The ten seconds are a give-up deadline, not a measurement: `ping` runs 60 s.

use std::time::Duration;

use super::*;
use crate::NoConsoleWindow as _;

/// How long a test waits for a child that should end at once.
const GIVE_UP: Duration = Duration::from_secs(10);

/// `ping` for a minute: a child that outlives every test unless its job ends it.
fn long_child() -> Command {
    let mut command = Command::new("ping.exe");
    command.args(["-n", "60", "127.0.0.1"]).kill_on_drop(true);
    command
}

/// Starts `command` suspended, puts it into a new job and lets it run, as `spawn_in_job` does.
fn start_in_job(mut command: Command) -> (Child, Job) {
    let child = spawn_suspended(&mut command).expect("the child starts suspended");
    let job = Job::adopt(&child).expect("the job takes the child");
    resume(&child).expect("the child runs");
    (child, job)
}

/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`: closing the job's last handle ends the child.
#[tokio::test]
async fn a_child_ends_with_its_job() {
    let (mut child, job) = start_in_job(long_child());
    let pid = child.id().expect("the child runs");
    assert_eq!(job.active(), Some(1));
    assert_eq!(job.pids(), vec![pid]);
    drop(job);
    tokio::time::timeout(GIVE_UP, child.wait())
        .await
        .expect("the child ended with its job")
        .expect("a status");
}

/// `terminate` ends every process of the job with [`TERMINATED`], which `ping` never returns.
#[tokio::test]
async fn a_terminated_job_ends_its_child_with_its_code() {
    let (mut child, job) = start_in_job(long_child());
    job.terminate().expect("the job is terminated");
    let status = tokio::time::timeout(GIVE_UP, child.wait())
        .await
        .expect("the child ended")
        .expect("a status");
    assert_eq!(status.code(), i32::try_from(TERMINATED).ok());
}

/// A child started suspended runs to its own end once resumed: its exit code comes back.
#[tokio::test]
async fn a_suspended_child_runs_once_resumed() {
    let mut command = Command::new("cmd.exe");
    command.args(["/C", "exit 7"]).kill_on_drop(true);
    let mut child = spawn_suspended(&mut command).expect("the child starts suspended");
    resume(&child).expect("its thread resumes");
    let status = tokio::time::timeout(GIVE_UP, child.wait())
        .await
        .expect("the child ran to its end")
        .expect("a status");
    assert_eq!(status.code(), Some(7));
}

/// A child that has ended has no handle any more: no job takes it, nothing is resumed.
#[tokio::test]
async fn an_ended_child_joins_no_job() {
    let mut command = Command::new("cmd.exe");
    command.args(["/C", "exit 0"]).no_console_window();
    let mut child = command.spawn().expect("the child starts");
    child.wait().await.expect("the child ends");
    assert!(Job::adopt(&child).is_err());
    assert!(resume(&child).is_err());
}
