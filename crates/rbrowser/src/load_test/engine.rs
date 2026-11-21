//! A run (RB-096-07): async on the core's runtime, apart from the UI thread and the views. A
//! controller follows the profile - every [`CONTROL_TICK`] it sets how many users may run and
//! starts the missing ones - closes each second into the series, judges the abort rules and
//! ends the run at its end, on an abort rule or on [`Halt::halt`] (Stop, rBrowser closing).
//! Each virtual user is a task of its own: it waits for its slot of the shared [`Pace`], picks
//! a step, sends it ([`Shot::fire`]), records the outcome and thinks; at every wait it also
//! listens for the end, so a stop never waits for a send or a pause. On the end the controller
//! has the shot end every connection at once ([`Shot::halt`]), and an answer that comes back
//! after it is not counted.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use rb_util::lock;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tokio::time::{Instant, MissedTickBehavior, interval, sleep, sleep_until};

use super::abort::{Abort, judge};
use super::pace::Pace;
use super::pick::Picker;
use super::profile::users_at;
use super::scenario::{AbortRules, Order, Profile, Think};
use super::stats::{Hit, Recorder};

/// How often the controller looks at the profile and the clock.
pub const CONTROL_TICK: Duration = Duration::from_millis(100);

/// How a run ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum End {
    /// Its time was up.
    Completed,
    /// A person stopped it.
    Stopped,
    /// An abort rule stopped it.
    Aborted(Abort),
    /// rBrowser closed.
    Closed,
}

impl End {
    /// The end's id in the report and the panel.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Stopped => "stopped",
            Self::Aborted(_) => "aborted",
            Self::Closed => "closed",
        }
    }

    /// The abort rule, when one ended it.
    #[must_use]
    pub const fn abort(self) -> Option<Abort> {
        match self {
            Self::Aborted(abort) => Some(abort),
            _ => None,
        }
    }
}

/// What ends a run: shared by the controller, its users and whoever may stop it.
#[derive(Clone)]
pub struct Halt {
    tx: Arc<watch::Sender<Option<End>>>,
}

impl Default for Halt {
    fn default() -> Self {
        Self {
            tx: Arc::new(watch::channel(None).0),
        }
    }
}

impl Halt {
    /// Ends the run with `end`; whether this was its end (the first one counts).
    pub fn halt(&self, end: End) -> bool {
        self.tx.send_if_modified(|now| {
            if now.is_none() {
                *now = Some(end);
                true
            } else {
                false
            }
        })
    }

    /// How the run ended, once it has.
    #[must_use]
    pub fn ended(&self) -> Option<End> {
        *self.tx.borrow()
    }

    fn watch(&self) -> watch::Receiver<Option<End>> {
        self.tx.subscribe()
    }
}

async fn halted(rx: &mut watch::Receiver<Option<End>>) {
    // The sender lives as long as the run: an error means it is gone, which is an end as well.
    let _ = rx.wait_for(Option::is_some).await;
}

/// Sends one request of a step.
#[async_trait]
pub trait Shot: Send + Sync {
    /// Step `step` for user `user` in its round `iteration`.
    async fn fire(&self, step: usize, user: u32, iteration: u64) -> Hit;
    /// Ends every connection at once.
    fn halt(&self);
}

/// What a run does.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub profile: Profile,
    /// The requests per second of all users together; none: unpaced.
    pub rps: Option<f64>,
    pub order: Order,
    /// Each step's weight and think time.
    pub weights: Vec<u32>,
    pub thinks: Vec<Think>,
    pub abort: AbortRules,
}

/// A closed second, as the live view hears of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub t: u32,
    pub elapsed_s: f64,
    /// The users running now.
    pub users: u32,
    /// The users the profile wants now.
    pub target: u32,
}

/// Hears of every closed second, with the run's numbers so far.
pub type OnSecond = Box<dyn Fn(&Recorder, &Tick) + Send + Sync>;

/// A run that ended.
pub struct Finished {
    pub end: End,
    pub elapsed_s: f64,
    pub recorder: Recorder,
}

struct Shared {
    plan: Plan,
    shot: Arc<dyn Shot>,
    halt: Halt,
    recorder: Mutex<Recorder>,
    pace: Option<Mutex<Pace>>,
    allowed: AtomicU32,
    active: AtomicU32,
}

async fn user_loop(shared: Arc<Shared>, user: u32) {
    shared.active.fetch_add(1, Ordering::AcqRel);
    let mut rx = shared.halt.watch();
    let mut picker = Picker::new(shared.plan.order, &shared.plan.weights, u64::from(user) + 1);
    let mut iteration = 0_u64;
    loop {
        if rx.borrow().is_some() || user >= shared.allowed.load(Ordering::Acquire) {
            break;
        }
        if let Some(pace) = &shared.pace {
            let at = lock(pace).slot(Instant::now());
            tokio::select! {
                () = sleep_until(at) => {}
                () = halted(&mut rx) => break,
            }
        }
        let step = picker.next_step();
        let hit = tokio::select! {
            hit = shared.shot.fire(step, user, iteration) => hit,
            () = halted(&mut rx) => break,
        };
        if rx.borrow().is_some() {
            break;
        }
        lock(&shared.recorder).record(&hit);
        iteration += 1;
        let think = picker.think(shared.plan.thinks.get(step).copied().unwrap_or_default());
        if !think.is_zero() {
            tokio::select! {
                () = sleep(think) => {}
                () = halted(&mut rx) => break,
            }
        }
    }
    shared.active.fetch_sub(1, Ordering::AcqRel);
}

fn close_second(shared: &Shared, t: u32, start: Instant, on_second: &OnSecond) -> Option<Abort> {
    let users = shared.active.load(Ordering::Acquire);
    let target = shared.allowed.load(Ordering::Acquire);
    let mut recorder = lock(&shared.recorder);
    recorder.tick(t, users);
    let tick = Tick {
        t,
        elapsed_s: start.elapsed().as_secs_f64(),
        users,
        target,
    };
    on_second(&recorder, &tick);
    judge(&shared.plan.abort, &recorder.window())
}

/// Runs `plan`, each request sent by `shot`, until its end or `halt`; `on_second` hears of every
/// closed second.
pub async fn run(plan: Plan, shot: Arc<dyn Shot>, halt: Halt, on_second: OnSecond) -> Finished {
    let steps = plan.weights.len();
    let shared = Arc::new(Shared {
        recorder: Mutex::new(Recorder::new(steps, plan.abort.window_s)),
        pace: plan.rps.and_then(Pace::new).map(Mutex::new),
        plan,
        shot,
        halt: halt.clone(),
        allowed: AtomicU32::new(0),
        active: AtomicU32::new(0),
    });
    let duration = Duration::from_secs(shared.plan.profile.duration_s());
    let start = Instant::now();
    let mut rx = halt.watch();
    let mut ticker = interval(CONTROL_TICK);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut users = JoinSet::new();
    let (mut spawned, mut next_second) = (0_u32, 1_u32);
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            () = halted(&mut rx) => break,
        }
        let elapsed = start.elapsed();
        let target = users_at(&shared.plan.profile, elapsed);
        shared.allowed.store(target, Ordering::Release);
        while spawned < target {
            users.spawn(user_loop(Arc::clone(&shared), spawned));
            spawned += 1;
        }
        while users.try_join_next().is_some() {}
        let mut end = None;
        while elapsed >= Duration::from_secs(u64::from(next_second)) && end.is_none() {
            end = close_second(&shared, next_second, start, &on_second).map(End::Aborted);
            next_second += 1;
        }
        if end.is_none() && elapsed >= duration {
            end = Some(End::Completed);
        }
        if let Some(end) = end {
            halt.halt(end);
            break;
        }
    }
    // Every connection closes now; the users end at their wait.
    shared.shot.halt();
    while users.join_next().await.is_some() {}
    let pending = lock(&shared.recorder).pending();
    if pending > 0 {
        let _ = close_second(&shared, next_second, start, &on_second);
    }
    let elapsed_s = start.elapsed().as_secs_f64();
    let end = halt.ended().unwrap_or(End::Stopped);
    let recorder = std::mem::replace(&mut *lock(&shared.recorder), Recorder::new(0, 1));
    Finished {
        end,
        elapsed_s,
        recorder,
    }
}

#[cfg(test)]
#[path = "engine_tests.rs"]
mod tests;
