use std::path::PathBuf;

use rb_ipc::VideoTarget;

use super::*;

fn job(recording: &str, file: &str, target: &str, to: VideoTarget) -> Job {
    Job {
        recording: recording.to_owned(),
        file: file.to_owned(),
        target: target.to_owned(),
        to,
        folder: PathBuf::from("/p/.rbrowser/out/video").join(recording),
        source: PathBuf::from("/p/.rbrowser/out/video")
            .join(recording)
            .join(file),
    }
}

const A: &str = "2026-11-16_14-03-07";
const B: &str = "2026-11-16_15-00-00";

/// RB-090-52: one job at a time, in order; the list shows the running one first with its share,
/// then those waiting.
#[test]
fn jobs_run_one_at_a_time_in_order() {
    let mut queue = Queue::default();
    assert_eq!(
        queue.push(job(A, "mobile.webm", "mobile.mp4", VideoTarget::Mp4)),
        Ok(0)
    );
    assert_eq!(
        queue.push(job(B, "desktop.mp4", "desktop.webm", VideoTarget::Webm)),
        Ok(1),
        "one ahead"
    );
    let (first, _cancel) = queue.next().expect("the first");
    assert_eq!(first.file, "mobile.webm");
    assert!(queue.next().is_none(), "one at a time");
    queue.progress(40);
    let list = queue.snapshot();
    assert_eq!(list.len(), 2);
    assert!(list[0].running);
    assert_eq!(list[0].percent, Some(40));
    assert_eq!(list[0].target, "mobile.mp4");
    assert!(!list[1].running);
    assert_eq!(list[1].percent, None);
    assert_eq!(list[1].to, VideoTarget::Webm);
    assert_eq!(queue.busy(), [A, B]);

    queue.done();
    let (second, _cancel) = queue.next().expect("the second");
    assert_eq!(second.recording, B);
    queue.done();
    assert!(queue.next().is_none());
    assert!(queue.snapshot().is_empty());
    assert!(queue.busy().is_empty());
}

#[test]
fn the_same_result_is_not_queued_twice() {
    let mut queue = Queue::default();
    queue
        .push(job(A, "mobile.webm", "mobile.mp4", VideoTarget::Mp4))
        .expect("queued");
    assert_eq!(
        queue.push(job(A, "mobile.webm", "mobile.mp4", VideoTarget::Mp4)),
        Err(ConvertError::Queued)
    );
    let (_running, _cancel) = queue.next().expect("runs");
    assert_eq!(
        queue.push(job(A, "mobile.webm", "mobile.mp4", VideoTarget::Mp4)),
        Err(ConvertError::Queued),
        "nor while it runs"
    );
    assert_eq!(
        queue.push(job(A, "tablet.webm", "tablet.mp4", VideoTarget::Mp4)),
        Ok(1)
    );
}

/// The app's end: what waits is dropped, the running job is told to stop, nothing new starts.
#[test]
fn closing_cancels_the_running_job_and_drops_the_rest() {
    let mut queue = Queue::default();
    queue
        .push(job(A, "mobile.webm", "mobile.mp4", VideoTarget::Mp4))
        .expect("queued");
    queue
        .push(job(B, "desktop.webm", "desktop.mp4", VideoTarget::Mp4))
        .expect("queued");
    let (_running, cancel) = queue.next().expect("runs");
    queue.close();
    assert!(*cancel.borrow(), "the running job is told to stop");
    queue.done();
    assert!(queue.next().is_none(), "the waiting job is gone");
    assert!(matches!(
        queue.push(job(A, "tablet.webm", "tablet.mp4", VideoTarget::Mp4)),
        Err(ConvertError::Ffmpeg(_))
    ));
}

#[test]
fn the_progress_notices_come_at_the_quarters() {
    assert_eq!(quarter_crossed(None, 10), None);
    assert_eq!(quarter_crossed(None, 30), Some(25));
    assert_eq!(quarter_crossed(Some(30), 45), None);
    assert_eq!(quarter_crossed(Some(45), 80), Some(75));
    assert_eq!(
        quarter_crossed(Some(80), 100),
        None,
        "the end has its own notice"
    );
}
