use std::sync::Mutex;

use super::*;
use crate::cdp::lock;

/// What the pre-pass asked of the page.
#[derive(Clone, Debug, PartialEq)]
enum Call {
    Step(Option<Scroll>, u32),
    Images(Duration),
}

/// A page `height` CSS px tall in a `viewport` that grows to `grows.1` once a stop reaches
/// `grows.0` (content loaded on the way), with `pending` images that never load.
struct FakePage {
    viewport: f64,
    height: Mutex<f64>,
    grows: Option<(f64, f64)>,
    at: Mutex<Scroll>,
    pending: u32,
    /// The page does not scroll (`overflow: hidden` on the document).
    frozen: bool,
    /// How long each scroll takes (virtual time).
    delay: Duration,
    /// The page answers nothing the pre-pass can read.
    mute: bool,
    calls: Mutex<Vec<Call>>,
}

impl FakePage {
    fn new(viewport: f64, height: f64, start: f64) -> Self {
        Self {
            viewport,
            height: Mutex::new(height),
            grows: None,
            at: Mutex::new(Scroll { x: 0.0, y: start }),
            pending: 0,
            frozen: false,
            delay: Duration::ZERO,
            mute: false,
            calls: Mutex::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<Call> {
        lock(&self.calls).clone()
    }

    /// The scroll tops of the walk, without the read before and the way back.
    fn stops(&self) -> Vec<f64> {
        self.calls()
            .into_iter()
            .filter_map(|call| match call {
                Call::Step(Some(to), STEP_MARKER_MS) => Some(to.y),
                _ => None,
            })
            .collect()
    }

    fn at(&self) -> Scroll {
        *lock(&self.at)
    }
}

#[async_trait]
impl LazyPage for FakePage {
    async fn step(&self, to: Option<Scroll>, marker_ms: u32) -> Result<Seen, EngineError> {
        lock(&self.calls).push(Call::Step(to, marker_ms));
        if self.mute {
            return Err(EngineError::Invalid("no agent".to_owned()));
        }
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        if let Some(to) = to.filter(|_| !self.frozen) {
            let height = *lock(&self.height);
            let y = to.y.clamp(0.0, (height - self.viewport).max(0.0));
            *lock(&self.at) = Scroll { x: to.x, y };
            if let Some((at, grown)) = self.grows
                && y >= at
            {
                *lock(&self.height) = grown;
            }
        }
        Ok(Seen {
            at: self.at(),
            height: *lock(&self.height),
            viewport: self.viewport,
        })
    }

    async fn images(&self, wait: Duration) -> Result<u32, EngineError> {
        lock(&self.calls).push(Call::Images(wait));
        Ok(self.pending)
    }
}

/// Every stop [`next_stop`] gives for a page that keeps its height.
fn plan(viewport: f64, height: f64) -> Vec<f64> {
    let mut stops = Vec::new();
    let mut at = None;
    while let Some(stop) = next_stop(at, viewport, height) {
        stops.push(stop);
        at = Some(stop);
        assert!(stops.len() <= MAX_STOPS, "the plan ends");
    }
    stops
}

#[test]
fn the_walk_goes_down_one_viewport_at_a_time_to_the_last_top() {
    assert_eq!(plan(852.0, 4000.5), [0.0, 852.0, 1704.0, 2556.0, 3148.5]);
    assert_eq!(plan(1000.0, 2000.0), [0.0, 1000.0]);
}

#[test]
fn a_page_within_its_viewport_needs_no_walk() {
    assert!(plan(800.0, 800.0).is_empty());
    assert!(plan(800.0, 500.0).is_empty());
    assert!(plan(800.0, 800.5).is_empty(), "less than a pixel to scroll");
}

#[test]
fn the_walk_ends_where_an_image_ends_and_never_without_a_viewport() {
    let stops = plan(1000.0, 1_000_000.0);
    assert_eq!(stops.last().copied(), Some(WALK_LIMIT - 1000.0));
    assert!(stops.len() <= MAX_STOPS);
    assert!(plan(0.0, 4000.0).is_empty());
    assert!(plan(f64::NAN, 4000.0).is_empty());
    assert!(plan(800.0, f64::INFINITY).is_empty());
}

#[tokio::test]
async fn the_prepass_walks_the_page_waits_for_its_images_and_returns() {
    let page = FakePage::new(852.0, 4000.5, 300.0);
    let done = prepass(&page, DEADLINE).await;
    let start = Scroll { x: 0.0, y: 300.0 };
    assert_eq!(
        done,
        Prepass {
            start: Some(start),
            timed_out: false
        }
    );
    assert_eq!(page.stops(), [0.0, 852.0, 1704.0, 2556.0, 3148.5]);
    let calls = page.calls();
    assert_eq!(calls[0], Call::Step(None, STEP_MARKER_MS), "read first");
    // Back where it was, the echo marker held over the shot, then the images.
    let back = calls.len() - 2;
    assert_eq!(calls[back], Call::Step(Some(start), SHOT_MARKER_MS));
    assert!(
        matches!(calls[back + 1], Call::Images(wait) if wait <= DEADLINE),
        "{calls:?}"
    );
    assert_eq!(page.at(), start);
}

#[tokio::test]
async fn content_loaded_on_the_way_lengthens_the_walk() {
    let mut page = FakePage::new(1000.0, 2000.0, 0.0);
    page.grows = Some((1000.0, 5000.0));
    let done = prepass(&page, DEADLINE).await;
    assert!(!done.timed_out);
    assert_eq!(page.stops(), [0.0, 1000.0, 2000.0, 3000.0, 4000.0]);
}

#[tokio::test]
async fn a_page_that_does_not_scroll_ends_the_walk_at_once() {
    let mut page = FakePage::new(800.0, 4000.0, 0.0);
    page.frozen = true;
    let done = prepass(&page, DEADLINE).await;
    assert!(!done.timed_out);
    assert_eq!(page.stops(), [0.0, 800.0]);
}

#[tokio::test]
async fn a_walk_cut_at_its_stops_still_waits_for_the_images_and_is_no_timeout() {
    // A tall page on a short breakpoint: more stops than a walk takes, nothing still loading.
    let page = FakePage::new(100.0, 100_000.0, 0.0);
    let done = prepass(&page, DEADLINE).await;
    assert!(!done.timed_out);
    assert_eq!(page.stops().len(), MAX_STOPS);
    assert!(matches!(page.calls().last(), Some(Call::Images(_))));
}

#[tokio::test]
async fn images_still_loading_at_the_deadline_mark_the_capture() {
    let mut page = FakePage::new(800.0, 4000.0, 0.0);
    page.pending = 2;
    let done = prepass(&page, DEADLINE).await;
    assert!(done.timed_out);
    assert_eq!(done.start, Some(Scroll { x: 0.0, y: 0.0 }));
}

#[tokio::test(start_paused = true)]
async fn a_walk_past_the_deadline_is_cut_and_still_returns() {
    let mut page = FakePage::new(800.0, 40_000.0, 120.0);
    page.delay = Duration::from_secs(1);
    let done = prepass(&page, Duration::from_secs(4)).await;
    assert!(done.timed_out);
    assert!(page.stops().len() < 10, "{:?}", page.stops());
    let calls = page.calls();
    assert!(
        !calls.iter().any(|call| matches!(call, Call::Images(_))),
        "no image wait after the deadline"
    );
    let start = Scroll { x: 0.0, y: 120.0 };
    assert_eq!(calls.last(), Some(&Call::Step(Some(start), SHOT_MARKER_MS)));
    assert_eq!(page.at(), start);
}

#[tokio::test]
async fn a_page_that_answers_nothing_is_left_alone() {
    let mut page = FakePage::new(800.0, 4000.0, 0.0);
    page.mute = true;
    let done = prepass(&page, DEADLINE).await;
    assert_eq!(done, Prepass::default());
    assert_eq!(page.calls().len(), 1);
}

#[tokio::test]
async fn put_back_scrolls_to_the_start_with_a_short_marker() {
    let page = FakePage::new(800.0, 4000.0, 0.0);
    let start = Scroll { x: 0.0, y: 2500.0 };
    put_back(&page, start).await;
    assert_eq!(page.calls(), [Call::Step(Some(start), STEP_MARKER_MS)]);
    assert_eq!(page.at(), start);
}

#[test]
fn only_a_capture_past_its_deadline_is_marked() {
    let image = rb_visual::Rgba::new(1, 1, [0, 0, 0, 255]).expect("small");
    let png = rb_visual::encode_png(&image).expect("encoded");
    let whole = Prepass {
        start: None,
        timed_out: false,
    };
    assert_eq!(whole.mark(png.clone()), png);
    let late = Prepass {
        start: None,
        timed_out: true,
    };
    let marked = late.mark(png.clone());
    assert!(rb_visual::images_pending(&marked));
    assert!(!rb_visual::images_pending(&png));
    // A PNG the note cannot go into is kept as it is.
    assert_eq!(late.mark(b"not a png".to_vec()), b"not a png");
}
