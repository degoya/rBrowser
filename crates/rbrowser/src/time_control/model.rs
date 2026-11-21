//! The page clock and the animation rates as data (RB-094-07), pure: the clock's start - an
//! instant, or a wall time each page reads in its own time zone - its speed and the real time it
//! runs from; the commands' arguments; the rates the shortcut steps through; what the activity
//! bar shows. The clock is one for every view (the views stay in sync); the rates are per view.

use rb_config::{ClockUserSettings, DEFAULT_ANIMATION_RATES, EmulationUserSettings};
use rb_engine::{Target, ViewId};
use rb_ipc::{ClockActivity, TimeControlActivity, ViewRate};
use serde::Deserialize;
use serde_json::{Value, json};
use time::format_description::StaticFormatDescription;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;
use time::{Date, OffsetDateTime, PlainDateTime};

/// The fastest clock: a hundred seconds per second.
pub const MAX_SPEED: f64 = 100.0;
/// The fastest animation rate.
pub const MAX_RATE: f64 = 10.0;
/// The longest step of stopped animations, in ms.
pub const MAX_STEP_MS: u32 = 60_000;
/// Two rates closer than this are one.
const SAME: f64 = 1e-9;
/// The forms `time` takes, for the errors.
const TIME_FORMS: &str = "an ISO 8601 date and time: 2026-12-24T10:00 (each page's local time), \
     2026-12-24T09:00:00Z or with an offset (one moment), 2026-12-24, or now";
/// The wall times taken, without a zone.
const LOCAL_FORMS: [StaticFormatDescription; 4] = [
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second][optional [.[subsecond]]]"),
    format_description!("[year]-[month]-[day]T[hour]:[minute]"),
    format_description!("[year]-[month]-[day] [hour]:[minute]:[second][optional [.[subsecond]]]"),
    format_description!("[year]-[month]-[day] [hour]:[minute]"),
];
/// A date alone: its midnight.
const DATE_FORM: StaticFormatDescription = format_description!("[year]-[month]-[day]");
/// An instant as the activity bar shows it: RFC 3339 in UTC, to the millisecond.
const INSTANT_TEXT: StaticFormatDescription =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:3]Z");

/// A wall time without a zone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1 to 12.
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millis: u32,
}

/// Where the clock starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockStart {
    /// One moment, ms since the epoch: every view reads the same instant.
    Instant(i64),
    /// A wall time each page reads in its own time zone, emulated or not: every view shows it.
    Local(LocalTime),
}

impl ClockStart {
    /// As the activity bar shows it: `2026-12-24T10:00:00` for a wall time, RFC 3339 in UTC
    /// for an instant.
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            ClockStart::Instant(ms) => {
                OffsetDateTime::from_unix_timestamp_nanos(i128::from(*ms) * 1_000_000)
                    .ok()
                    .and_then(|time| time.format(INSTANT_TEXT).ok())
                    .unwrap_or_else(|| ms.to_string())
            }
            ClockStart::Local(time) => {
                let mut text = format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
                    time.year, time.month, time.day, time.hour, time.minute, time.second
                );
                if time.millis > 0 {
                    text.push_str(&format!(".{:03}", time.millis));
                }
                text
            }
        }
    }
}

/// The views' clock: at the real time `anchor_ms` it reads `start` plus `offset_ms`, and runs
/// `speed` times as fast as the real time from there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub start: ClockStart,
    /// What ran before the last change of speed, in ms.
    pub offset_ms: f64,
    /// The real time, ms since the epoch, it runs from.
    pub anchor_ms: i64,
    /// 0 holds it, 1 is real time.
    pub speed: f64,
}

impl Clock {
    /// A clock at `start` from the real time `now_ms` on.
    #[must_use]
    pub fn starting(start: ClockStart, speed: f64, now_ms: i64) -> Self {
        Self {
            start,
            offset_ms: 0.0,
            anchor_ms: now_ms,
            speed,
        }
    }

    /// The same clock at `speed` from the real time `now_ms` on: what ran so far stays.
    #[must_use]
    #[allow(
        clippy::cast_precision_loss,
        reason = "a real time span in ms: far below 2^52, exact as f64"
    )]
    pub fn with_speed(self, speed: f64, now_ms: i64) -> Self {
        let ran = now_ms.saturating_sub(self.anchor_ms) as f64 * self.speed;
        Self {
            offset_ms: self.offset_ms + ran,
            anchor_ms: now_ms,
            speed,
            ..self
        }
    }

    /// The page script's configuration (`clock.js`'s `set`): data only.
    #[must_use]
    pub fn config(&self) -> Value {
        let mut config = json!({
            "offset": self.offset_ms,
            "anchor": self.anchor_ms,
            "speed": self.speed,
        });
        match self.start {
            ClockStart::Instant(ms) => config["origin"] = json!(ms),
            ClockStart::Local(time) => {
                config["local"] = json!([
                    time.year,
                    time.month,
                    time.day,
                    time.hour,
                    time.minute,
                    time.second,
                    time.millis
                ]);
            }
        }
        config
    }

    /// What the activity bar shows of it.
    #[must_use]
    pub fn activity(&self) -> ClockActivity {
        ClockActivity {
            start: self.start.text(),
            local: matches!(self.start, ClockStart::Local(_)),
            speed: self.speed,
        }
    }
}

/// Reads `text` as the clock's start; empty or `now` is the real time `now_ms`.
///
/// # Errors
///
/// No date and time of the forms taken, or a year outside 1 to 9999.
pub fn parse_time(text: &str, now_ms: i64) -> Result<ClockStart, String> {
    let text = text.trim();
    if text.is_empty() || text.eq_ignore_ascii_case("now") {
        return Ok(ClockStart::Instant(now_ms));
    }
    let refused = || format!("`{text}` is no time; give {TIME_FORMS}");
    if let Ok(time) = OffsetDateTime::parse(&rfc3339_form(text), &Rfc3339) {
        return if (1..=9999).contains(&time.year()) {
            Ok(ClockStart::Instant(millis_of(time)))
        } else {
            Err(refused())
        };
    }
    let naive = LOCAL_FORMS
        .iter()
        .find_map(|form| PlainDateTime::parse(text, form).ok())
        .or_else(|| Date::parse(text, DATE_FORM).ok().map(Date::midnight))
        .filter(|time| (1..=9999).contains(&time.year()))
        .ok_or_else(refused)?;
    Ok(ClockStart::Local(LocalTime {
        year: naive.year(),
        month: u32::from(u8::from(naive.month())),
        day: u32::from(naive.day()),
        hour: u32::from(naive.hour()),
        minute: u32::from(naive.minute()),
        second: u32::from(naive.second()),
        millis: u32::from(naive.millisecond()),
    }))
}

/// `text` with a space or `t` between date and time as the `T` the `time` crate's RFC 3339
/// wants; chrono read all three.
fn rfc3339_form(text: &str) -> std::borrow::Cow<'_, str> {
    match (text.get(..10), text.get(10..11), text.get(11..)) {
        (Some(date), Some(" " | "t"), Some(rest)) => format!("{date}T{rest}").into(),
        _ => text.into(),
    }
}

/// `time` in ms since the epoch, rounded down as JavaScript's `Date` counts them.
#[must_use]
pub fn millis_of(time: OffsetDateTime) -> i64 {
    i64::try_from(time.unix_timestamp_nanos().div_euclid(1_000_000))
        .expect("the years 1 to 9999 in ms fit an i64")
}

/// `speed` when it is one.
///
/// # Errors
///
/// Not a number from 0 to [`MAX_SPEED`].
pub fn check_speed(speed: f64) -> Result<f64, String> {
    if speed.is_finite() && (0.0..=MAX_SPEED).contains(&speed) {
        Ok(speed)
    } else {
        Err(format!(
            "`speed` is from 0 (fixed) to {MAX_SPEED}; 1 is real time"
        ))
    }
}

/// What `emulation.clock` asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum ClockRequest {
    /// The shortcut: the running clock goes, else the settings' clock comes.
    Toggle,
    /// The real time again.
    Reset,
    /// A clock from `start` (absent: the running clock's time, else the real time) at `speed`
    /// (absent: the running clock's, else the settings').
    Set {
        start: Option<ClockStart>,
        speed: Option<f64>,
    },
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawClock {
    #[serde(default)]
    time: Option<Value>,
    #[serde(default)]
    speed: Option<f64>,
    #[serde(default)]
    reset: Option<bool>,
}

/// Reads `emulation.clock`'s arguments: `{ time?, speed?, reset? }`, none the shortcut's toggle.
///
/// # Errors
///
/// Unknown fields, `reset` with a time or a speed, a time or a speed that is none.
pub fn parse_clock(args: &Value, now_ms: i64) -> Result<ClockRequest, String> {
    let raw: RawClock = if args.is_null() {
        RawClock::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| error.to_string())?
    };
    match raw.reset {
        Some(true) if raw.time.is_some() || raw.speed.is_some() => {
            return Err("`reset` stands alone: no `time`, no `speed`".to_owned());
        }
        Some(true) => return Ok(ClockRequest::Reset),
        Some(false) => return Err("`reset` is true or absent".to_owned()),
        None => {}
    }
    let speed = raw.speed.map(check_speed).transpose()?;
    let start = match raw.time {
        None => None,
        Some(Value::String(text)) => Some(parse_time(&text, now_ms)?),
        Some(Value::Number(number)) => {
            Some(ClockStart::Instant(number.as_i64().ok_or_else(|| {
                "`time` as a number is whole ms since the epoch".to_owned()
            })?))
        }
        Some(_) => return Err(format!("`time` is {TIME_FORMS}, or ms since the epoch")),
    };
    Ok(match (start, speed) {
        (None, None) => ClockRequest::Toggle,
        (start, speed) => ClockRequest::Set { start, speed },
    })
}

/// The clock after `request`, with `current` running and `defaults` the user's settings;
/// `None` is the real time.
///
/// # Errors
///
/// `emulation.clock.default` or `emulation.clock.speed` is broken.
pub fn next_clock(
    request: &ClockRequest,
    current: Option<&Clock>,
    defaults: &ClockUserSettings,
    now_ms: i64,
) -> Result<Option<Clock>, String> {
    let default_speed =
        || check_speed(defaults.speed).map_err(|error| format!("emulation.clock.speed: {error}"));
    Ok(match (request, current) {
        (ClockRequest::Reset, _) | (ClockRequest::Toggle, Some(_)) => None,
        (ClockRequest::Toggle, None) => {
            let start = parse_time(&defaults.default, now_ms)
                .map_err(|error| format!("emulation.clock.default: {error}"))?;
            Some(Clock::starting(start, default_speed()?, now_ms))
        }
        (
            ClockRequest::Set {
                start: Some(start),
                speed,
            },
            current,
        ) => {
            let speed = match (speed, current) {
                (Some(speed), _) => *speed,
                (None, Some(running)) => running.speed,
                (None, None) => default_speed()?,
            };
            Some(Clock::starting(*start, speed, now_ms))
        }
        (ClockRequest::Set { start: None, speed }, Some(running)) => {
            Some(running.with_speed(speed.unwrap_or(running.speed), now_ms))
        }
        (ClockRequest::Set { start: None, speed }, None) => Some(Clock::starting(
            ClockStart::Instant(now_ms),
            match speed {
                Some(speed) => *speed,
                None => default_speed()?,
            },
            now_ms,
        )),
    })
}

/// What `overlays.animations` asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum AnimationRequest {
    /// The shortcut: the next of the settings' rates.
    Cycle,
    /// This rate: 0 stops, 1 is normal.
    Rate(f64),
    /// Stopped animations move this many ms on; `None`: the settings' step.
    Step(Option<u32>),
}

/// `overlays.animations`' arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationArgs {
    /// `None`: every view.
    pub target: Option<Target>,
    pub request: AnimationRequest,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnimations {
    #[serde(default)]
    target: Option<Target>,
    #[serde(default)]
    rate: Option<f64>,
    #[serde(default)]
    step: Option<Value>,
}

/// Reads `overlays.animations`' arguments: `{ target?, rate? | step? }`, none the shortcut's
/// next rate; `step` is `true` (the settings' step) or ms.
///
/// # Errors
///
/// Unknown fields, `rate` with `step`, a rate outside 0 to [`MAX_RATE`], a step that is none.
pub fn parse_animations(args: &Value) -> Result<AnimationArgs, String> {
    let raw: RawAnimations = if args.is_null() {
        RawAnimations::default()
    } else {
        serde_json::from_value(args.clone()).map_err(|error| error.to_string())?
    };
    let request = match (raw.rate, raw.step) {
        (Some(_), Some(_)) => return Err("`rate` or `step`, not both".to_owned()),
        (Some(rate), None) if rate.is_finite() && (0.0..=MAX_RATE).contains(&rate) => {
            AnimationRequest::Rate(rate)
        }
        (Some(_), None) => {
            return Err(format!(
                "`rate` is from 0 (stopped) to {MAX_RATE}; 1 is normal"
            ));
        }
        (None, Some(Value::Bool(true))) => AnimationRequest::Step(None),
        (None, Some(Value::Number(ms))) => match ms.as_u64().map(u32::try_from) {
            Some(Ok(ms)) if (1..=MAX_STEP_MS).contains(&ms) => AnimationRequest::Step(Some(ms)),
            _ => return Err(format!("`step` is from 1 to {MAX_STEP_MS} ms")),
        },
        (None, Some(_)) => {
            return Err(format!(
                "`step` is true (the settings' step) or 1 to {MAX_STEP_MS} ms"
            ));
        }
        (None, None) => AnimationRequest::Cycle,
    };
    Ok(AnimationArgs {
        target: raw.target,
        request,
    })
}

/// Whether `rate` is the normal speed.
#[must_use]
pub fn is_normal(rate: f64) -> bool {
    (rate - 1.0).abs() < SAME
}

/// The settings' rates the shortcut steps through: each once, from 0 to [`MAX_RATE`], in their
/// order, 100 % among them (the way back); the defaults for an empty list.
#[must_use]
pub fn rates_of(settings: &[f64]) -> Vec<f64> {
    let mut rates: Vec<f64> = Vec::new();
    for rate in settings {
        if rate.is_finite()
            && (0.0..=MAX_RATE).contains(rate)
            && !rates.iter().any(|seen| (seen - rate).abs() < SAME)
        {
            rates.push(*rate);
        }
    }
    if rates.is_empty() {
        return DEFAULT_ANIMATION_RATES.to_vec();
    }
    if !rates.iter().any(|rate| is_normal(*rate)) {
        rates.push(1.0);
    }
    rates
}

/// The rate after `current` in `rates`; the first one when `current` is none of them.
#[must_use]
pub fn next_rate(current: f64, rates: &[f64]) -> f64 {
    let at = rates.iter().position(|rate| (rate - current).abs() < SAME);
    let next = at.map_or(0, |at| (at + 1) % rates.len().max(1));
    rates.get(next).copied().unwrap_or(1.0)
}

/// The step in ms: the given one, else the settings', at least 1 and at most [`MAX_STEP_MS`].
#[must_use]
pub fn step_ms(step: Option<u32>, settings: &EmulationUserSettings) -> u32 {
    step.unwrap_or(settings.animation_step_ms)
        .clamp(1, MAX_STEP_MS)
}

/// What the activity bar shows: the clock while one runs, the views not at 100 % in view id
/// order; `None` while nothing is changed.
#[must_use]
pub fn activity(clock: Option<&Clock>, rates: &[(ViewId, f64)]) -> Option<TimeControlActivity> {
    let mut animations: Vec<ViewRate> = rates
        .iter()
        .filter(|(_, rate)| !is_normal(*rate))
        .map(|(view, rate)| ViewRate {
            view: *view,
            rate: *rate,
        })
        .collect();
    animations.sort_by_key(|entry| entry.view);
    let shown = TimeControlActivity {
        clock: clock.map(Clock::activity),
        animations,
    };
    (!shown.is_empty()).then_some(shown)
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
