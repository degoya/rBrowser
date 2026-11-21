use std::sync::atomic::{AtomicBool, AtomicU64};

use super::*;
use crate::load_test::stats::Failed;

struct Fake {
    latency: Duration,
    status: u16,
    halted: AtomicBool,
    fired: AtomicU64,
}

impl Fake {
    fn new(latency_ms: u64, status: u16) -> Arc<Self> {
        Arc::new(Self {
            latency: Duration::from_millis(latency_ms),
            status,
            halted: AtomicBool::new(false),
            fired: AtomicU64::new(0),
        })
    }
}

#[async_trait]
impl Shot for Fake {
    async fn fire(&self, step: usize, _user: u32, _iteration: u64) -> Hit {
        self.fired.fetch_add(1, Ordering::SeqCst);
        sleep(self.latency).await;
        Hit {
            step,
            ms: self.latency.as_secs_f64() * 1000.0,
            status: Some(self.status),
            failed: Failed::of_status(self.status),
        }
    }

    fn halt(&self) {
        self.halted.store(true, Ordering::SeqCst);
    }
}

fn plan(users: u32, up: u32, hold: u32, down: u32, rps: Option<f64>) -> Plan {
    Plan {
        profile: Profile {
            users,
            ramp_up_s: up,
            hold_s: hold,
            ramp_down_s: down,
            rps,
        },
        rps,
        order: Order::Sequence,
        weights: vec![1],
        thinks: vec![Think::default()],
        abort: AbortRules::default(),
    }
}

fn quiet() -> OnSecond {
    Box::new(|_, _| {})
}

#[tokio::test(start_paused = true)]
async fn a_constant_run_keeps_its_users_busy_until_its_end() {
    let fake = Fake::new(100, 200);
    let finished = run(
        plan(5, 0, 3, 0, None),
        fake.clone(),
        Halt::default(),
        quiet(),
    )
    .await;
    assert_eq!(finished.end, End::Completed);
    let requests = finished.recorder.totals(finished.elapsed_s).requests;
    // Five users, ten requests a second each, three seconds.
    assert!((140..=160).contains(&requests), "{requests}");
    assert_eq!(finished.recorder.series().len(), 3);
    assert!(
        fake.halted.load(Ordering::SeqCst),
        "connections closed at the end"
    );
}

#[tokio::test(start_paused = true)]
async fn a_ramp_climbs_holds_and_falls() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let noted = Arc::clone(&seen);
    let on_second: OnSecond = Box::new(move |_, tick| lock(&noted).push(tick.users));
    let finished = run(
        plan(4, 4, 2, 4, None),
        Fake::new(50, 200),
        Halt::default(),
        on_second,
    )
    .await;
    assert_eq!(finished.end, End::Completed);
    let users = lock(&seen).clone();
    let top = users.iter().copied().max().unwrap_or_default();
    assert_eq!(top, 4, "{users:?}");
    let peak = users.iter().position(|n| *n == 4).unwrap_or_default();
    assert!(users[..peak].windows(2).all(|w| w[0] <= w[1]), "{users:?}");
    assert!(users[peak..].windows(2).all(|w| w[0] >= w[1]), "{users:?}");
    assert!(users.first().copied().unwrap_or_default() <= 2, "{users:?}");
}

#[tokio::test(start_paused = true)]
async fn the_rate_holds_all_users_together() {
    let finished = run(
        plan(5, 0, 3, 0, Some(10.0)),
        Fake::new(10, 200),
        Halt::default(),
        quiet(),
    )
    .await;
    let requests = finished.recorder.totals(finished.elapsed_s).requests;
    // Ten a second for three seconds, whatever five users could do.
    assert!((28..=32).contains(&requests), "{requests}");
}

#[tokio::test(start_paused = true)]
async fn stop_ends_the_run_at_once_without_waiting_for_a_send() {
    let fake = Fake::new(30_000, 200);
    let halt = Halt::default();
    let running = tokio::spawn(run(
        plan(3, 0, 600, 0, None),
        fake.clone(),
        halt.clone(),
        quiet(),
    ));
    sleep(Duration::from_secs(2)).await;
    assert!(halt.halt(End::Stopped));
    assert!(!halt.halt(End::Completed), "the first end counts");
    let finished = running.await.expect("ran");
    assert_eq!(finished.end, End::Stopped);
    assert!(
        fake.halted.load(Ordering::SeqCst),
        "every connection closed"
    );
    assert!(finished.elapsed_s < 3.0, "{}", finished.elapsed_s);
    // The sends in flight never came back: none is counted.
    assert_eq!(finished.recorder.totals(finished.elapsed_s).requests, 0);
    assert_eq!(fake.fired.load(Ordering::SeqCst), 3);
}

#[tokio::test(start_paused = true)]
async fn an_abort_rule_stops_the_run() {
    let mut plan = plan(5, 0, 60, 0, None);
    plan.abort = AbortRules {
        error_rate: Some(0.5),
        p95_ms: None,
        window_s: 2,
        min_requests: 5,
    };
    let finished = run(plan, Fake::new(100, 503), Halt::default(), quiet()).await;
    let abort = finished.end.abort().expect("aborted");
    assert_eq!(abort.rule, "errorRate");
    assert_eq!(finished.end.id(), "aborted");
    assert!(finished.elapsed_s < 5.0, "{}", finished.elapsed_s);
}
