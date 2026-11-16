use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use serde_json::json;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::cdp::lock;

/// Keeps what reached it.
#[derive(Clone, Default)]
struct MemorySink {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
    finished: Arc<Mutex<bool>>,
}

#[async_trait]
impl FrameSink for MemorySink {
    async fn write(&mut self, jpeg: &[u8]) -> std::io::Result<()> {
        lock(&self.frames).push(jpeg.to_vec());
        Ok(())
    }

    async fn finish(self: Box<Self>) -> Result<(), String> {
        *lock(&self.finished) = true;
        Ok(())
    }
}

fn plan(view: u32, session: &str) -> ViewPlan {
    ViewPlan {
        view: ViewId(view),
        session: session.to_owned(),
        width: 780,
        height: 1688,
        file: PathBuf::from(format!("/out/{view}.mp4")),
    }
}

fn options(fps: u32, max_duration: Duration, now: f64) -> RecorderOptions {
    RecorderOptions {
        fps,
        quality: 80,
        max_duration,
        clock: Arc::new(move || now),
        stop_timeout: Duration::from_secs(10),
    }
}

const MINUTE: Duration = Duration::from_secs(60);

fn frame(fake: &FakeBrowser, session: &str, number: u64, timestamp: f64, bytes: &[u8]) {
    fake.emit(json!({
        "method": "Page.screencastFrame",
        "sessionId": session,
        "params": {
            "data": base64::engine::general_purpose::STANDARD.encode(bytes),
            "sessionId": number,
            "metadata": { "timestamp": timestamp, "deviceWidth": 390, "deviceHeight": 844,
                          "pageScaleFactor": 1, "offsetTop": 0, "scrollOffsetX": 0,
                          "scrollOffsetY": 0 },
        },
    }));
}

/// Waits until `fake` was sent `count` messages of `method`.
async fn sent(fake: &FakeBrowser, method: &str, count: usize) {
    for _ in 0..200 {
        if fake.sent(method).len() >= count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!(
        "{method} was sent {} times, not {count}",
        fake.sent(method).len()
    );
}

#[tokio::test]
async fn frames_are_acked_paced_and_written_per_view() {
    let (fake, router) = FakeBrowser::connect();
    let first = MemorySink::default();
    let second = MemorySink::default();
    let recording = Recording::start(
        router,
        vec![
            (plan(1, "S1"), Box::new(first.clone()) as Box<dyn FrameSink>),
            (plan(2, "S2"), Box::new(second.clone())),
        ],
        options(10, MINUTE, 101.0),
    )
    .await;
    let starts = fake.sent("Page.startScreencast");
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0]["sessionId"], "S1");
    assert_eq!(starts[0]["params"]["format"], "jpeg");
    assert_eq!(starts[0]["params"]["quality"], 80);
    assert_eq!(starts[0]["params"]["maxWidth"], 780);
    assert_eq!(starts[0]["params"]["maxHeight"], 1688);

    frame(&fake, "S1", 1, 100.0, b"a");
    frame(&fake, "S1", 2, 100.5, b"b");
    frame(&fake, "S2", 1, 100.0, b"x");
    // A frame of a session that is not recorded is neither taken nor acked.
    frame(&fake, "S9", 1, 100.0, b"z");
    sent(&fake, "Page.screencastFrameAck", 3).await;

    let finished = recording.stop().await;
    let acks = fake.sent("Page.screencastFrameAck");
    assert_eq!(acks.len(), 3, "{acks:?}");
    assert!(
        acks.iter()
            .any(|ack| ack["sessionId"] == "S1" && ack["params"]["sessionId"] == 2)
    );
    assert_eq!(fake.sent("Page.stopScreencast").len(), 2);

    // 10 fps from 100.0 to the stop at 101.0: five slots of `a`, five of `b`.
    let written = lock(&first.frames).clone();
    assert_eq!(written.len(), 10);
    assert_eq!(written.iter().filter(|f| f.as_slice() == b"a").count(), 5);
    assert_eq!(written.iter().filter(|f| f.as_slice() == b"b").count(), 5);
    // A page that painted once is still a file of the recorded length.
    assert_eq!(lock(&second.frames).len(), 10);
    assert!(*lock(&first.finished) && *lock(&second.finished));

    assert_eq!(finished.len(), 2);
    assert_eq!(finished[0].view, ViewId(1));
    assert_eq!(finished[0].frames, 10);
    assert!((finished[0].seconds - 1.0).abs() < 1e-9);
    assert_eq!(finished[0].error, None);
}

#[tokio::test]
async fn a_view_that_paints_nothing_is_named_in_the_result() {
    let (_fake, router) = FakeBrowser::connect();
    let sink = MemorySink::default();
    let recording = Recording::start(
        router,
        vec![(plan(1, "S1"), Box::new(sink.clone()) as Box<dyn FrameSink>)],
        options(30, MINUTE, 10.0),
    )
    .await;
    let finished = recording.stop().await;
    assert_eq!(finished[0].frames, 0);
    assert!(finished[0].error.is_some());
    assert!(*lock(&sink.finished), "the encoder is closed anyway");
}

/// `maxDurationSec` ends the recording by itself, and the files are finished.
#[tokio::test]
async fn the_maximum_duration_stops_the_recording() {
    let (fake, router) = FakeBrowser::connect();
    let sink = MemorySink::default();
    let mut recording = Recording::start(
        router,
        vec![(plan(1, "S1"), Box::new(sink.clone()) as Box<dyn FrameSink>)],
        options(10, Duration::from_millis(200), 1000.0),
    )
    .await;
    frame(&fake, "S1", 1, 998.0, b"a");
    let finished = recording.finished().await;
    assert_eq!(fake.sent("Page.stopScreencast").len(), 1);
    assert_eq!(finished[0].frames, 2, "0.2 s at 10 fps, never more");
    assert!(*lock(&sink.finished));
}

#[tokio::test]
async fn a_failed_start_is_reported_for_its_view() {
    let (fake, router) = FakeBrowser::connect();
    fake.fail("Page.startScreencast", true);
    let sink = MemorySink::default();
    let recording = Recording::start(
        router,
        vec![(plan(1, "S1"), Box::new(sink.clone()) as Box<dyn FrameSink>)],
        options(30, MINUTE, 10.0),
    )
    .await;
    let finished = recording.stop().await;
    let error = finished[0].error.clone().expect("an error");
    assert!(error.contains("startScreencast"), "{error}");
}

/// Takes no frame and never finishes: a hung encoder.
struct HungSink;

#[async_trait]
impl FrameSink for HungSink {
    async fn write(&mut self, _jpeg: &[u8]) -> std::io::Result<()> {
        std::future::pending().await
    }

    async fn finish(self: Box<Self>) -> Result<(), String> {
        std::future::pending().await
    }
}

/// Charge 9 review M1: a writer that hangs does not hold the recording - the stop returns after
/// the stop timeout with an error for that view and the others' files.
#[tokio::test]
async fn a_hung_writer_is_given_up_after_the_stop_timeout() {
    let (fake, router) = FakeBrowser::connect();
    let good = MemorySink::default();
    let mut options = options(10, MINUTE, 1000.0);
    options.stop_timeout = Duration::from_millis(300);
    let recording = Recording::start(
        router,
        vec![
            (plan(1, "S1"), Box::new(HungSink) as Box<dyn FrameSink>),
            (plan(2, "S2"), Box::new(good.clone())),
        ],
        options,
    )
    .await;
    frame(&fake, "S1", 1, 999.0, b"a");
    frame(&fake, "S2", 1, 999.0, b"b");
    sent(&fake, "Page.screencastFrameAck", 2).await;
    let finished = tokio::time::timeout(Duration::from_secs(10), recording.stop())
        .await
        .expect("the stop returns");
    assert_eq!(finished.len(), 2);
    let hung = finished
        .iter()
        .find(|file| file.view == ViewId(1))
        .expect("the hung view");
    let error = hung.error.clone().expect("an error");
    assert!(error.contains("did not finish"), "{error}");
    assert_eq!(hung.path, "/out/1.mp4");
    let fine = finished
        .iter()
        .find(|file| file.view == ViewId(2))
        .expect("the other view");
    assert_eq!(fine.error, None);
    assert!(*lock(&good.finished));
}

/// RB-090-52: an encoder that takes no frame never holds the page - every frame is acked at
/// once - and the frames its writer could not take are counted in the result.
#[tokio::test]
async fn a_slow_encoder_never_stalls_the_acks_and_its_drops_are_counted() {
    let (fake, router) = FakeBrowser::connect();
    let good = MemorySink::default();
    let mut options = options(10, MINUTE, 1000.0);
    options.stop_timeout = Duration::from_millis(300);
    let recording = Recording::start(
        router,
        vec![
            (plan(1, "S1"), Box::new(HungSink) as Box<dyn FrameSink>),
            (plan(2, "S2"), Box::new(good.clone())),
        ],
        options,
    )
    .await;
    const FRAMES: u64 = 30;
    for number in 0..FRAMES {
        let at = 990.0 + number as f64 * 0.1;
        frame(&fake, "S1", number, at, b"a");
    }
    frame(&fake, "S2", 1, 999.0, b"b");
    // Every frame of the hung view is acked: Chromium keeps sending, the page never waits.
    sent(&fake, "Page.screencastFrameAck", FRAMES as usize + 1).await;
    let finished = tokio::time::timeout(Duration::from_secs(10), recording.stop())
        .await
        .expect("the stop returns");
    let hung = finished
        .iter()
        .find(|file| file.view == ViewId(1))
        .expect("the hung view");
    // The writer holds at most two (one taken, one being written) and its inbox the capacity.
    let kept = WRITER_CAPACITY as u64 + 2;
    assert!(
        hung.dropped >= FRAMES - kept && hung.dropped <= FRAMES,
        "{} dropped",
        hung.dropped
    );
    let fine = finished
        .iter()
        .find(|file| file.view == ViewId(2))
        .expect("the other view");
    assert_eq!(fine.dropped, 0);
    assert_eq!(fine.error, None);
}

/// The stop result names the dropped frames.
#[test]
fn the_result_carries_the_dropped_frames() {
    let file = RecordedFile {
        view: ViewId(1),
        path: "/out/1.webm".to_owned(),
        frames: 30,
        seconds: 1.0,
        dropped: 4,
        error: None,
    };
    let value = serde_json::to_value(&file).expect("serializes");
    assert_eq!(value["dropped"], 4);
}
