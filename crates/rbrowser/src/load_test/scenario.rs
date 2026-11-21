//! A load test's scenario (RB-096-07): saved requests of the API client (`.rbrowser/api/`, by
//! id) in order or by weight, a think time after each, values per virtual user, the profile
//! (users, ramp up, hold, ramp down, a target rate) and the abort rules. One readable JSON file
//! per scenario below `.rbrowser/api/load/` ([`super::store`]), checked in like the requests.
//!
//! ```json
//! { "name": "Checkout", "order": "sequence",
//!   "steps": [{ "request": "shop/list.http", "weight": 3, "thinkMs": { "min": 500, "max": 1500 } }],
//!   "variables": { "user": ["ada", "bob"] },
//!   "profile": { "users": 50, "rampUpS": 10, "holdS": 40, "rampDownS": 10, "rps": 100 },
//!   "abort": { "errorRate": 0.2, "p95Ms": 2000 } }
//! ```
//!
//! A user's values: user `n` takes the `n`th value of each variable, round again when there are
//! fewer values than users; `{{name}}` (or `{{ name }}`) in every text of a step's request takes
//! it, `{{$vu}}` the user's number and `{{$iteration}}` its round, before the API client puts in
//! the environment's variables ([`personalize`]). The values are the project's own text, no
//! secrets: a secret stays a variable of the environment.

use std::collections::BTreeMap;

use rb_api_model::ApiRequest;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The abort rules' window, in seconds, when the file names none.
pub const DEFAULT_WINDOW_S: u32 = 10;
/// The fewest requests in the window before an abort rule judges.
pub const DEFAULT_MIN_REQUESTS: u32 = 20;
/// The most steps of one scenario.
pub const MAX_STEPS: usize = 100;

/// How a user picks its next step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Order {
    /// One after the other, then from the start again.
    #[default]
    Sequence,
    /// At random, each step as often as its weight says.
    Weighted,
}

/// A pause after a step, drawn between `min` and `max` milliseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Think {
    #[serde(default)]
    pub min: u64,
    #[serde(default)]
    pub max: u64,
}

/// One step: a saved request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Step {
    /// The saved request's id (`shop/list.http`).
    pub request: String,
    #[serde(default = "one")]
    pub weight: u32,
    #[serde(default)]
    pub think_ms: Think,
}

const fn one() -> u32 {
    1
}

/// How many users, how long, how fast.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    /// The users at the top: constant while holding.
    pub users: u32,
    /// Seconds from no user to `users` (0: all at once).
    #[serde(default)]
    pub ramp_up_s: u32,
    /// Seconds at `users`: the whole run of a constant profile.
    #[serde(default)]
    pub hold_s: u32,
    /// Seconds from `users` back to none (0: all stop at once).
    #[serde(default)]
    pub ramp_down_s: u32,
    /// The requests per second of all users together, at most; none: as fast as the users
    /// go, held to `load-test.maxRps`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rps: Option<f64>,
}

impl Profile {
    /// The run's length in seconds.
    #[must_use]
    pub fn duration_s(&self) -> u64 {
        u64::from(self.ramp_up_s) + u64::from(self.hold_s) + u64::from(self.ramp_down_s)
    }
}

/// When a run stops by itself before its end.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AbortRules {
    /// The share of failed requests (0 to 1) in the window that stops the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_rate: Option<f64>,
    /// The p95 latency in the window, in milliseconds, that stops the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub p95_ms: Option<f64>,
    /// The window's seconds.
    #[serde(default = "window")]
    pub window_s: u32,
    /// The fewest requests in the window before a rule judges.
    #[serde(default = "min_requests")]
    pub min_requests: u32,
}

const fn window() -> u32 {
    DEFAULT_WINDOW_S
}

const fn min_requests() -> u32 {
    DEFAULT_MIN_REQUESTS
}

impl Default for AbortRules {
    fn default() -> Self {
        Self {
            error_rate: None,
            p95_ms: None,
            window_s: DEFAULT_WINDOW_S,
            min_requests: DEFAULT_MIN_REQUESTS,
        }
    }
}

/// A scenario file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scenario {
    #[serde(default)]
    pub name: String,
    /// The API client's environment the requests run in; none: the one the views show.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    #[serde(default)]
    pub order: Order,
    pub steps: Vec<Step>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variables: BTreeMap<String, Vec<String>>,
    pub profile: Profile,
    #[serde(default)]
    pub abort: AbortRules,
}

impl Scenario {
    /// The scenario of a file's `text`.
    ///
    /// # Errors
    ///
    /// The reason, as serde gives it (the unknown field named).
    pub fn from_json(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|error| error.to_string())
    }

    /// The file's text: pretty, so a diff reads.
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut text = serde_json::to_string_pretty(self).unwrap_or_default();
        text.push('\n');
        text
    }

    /// Whether the scenario holds; else the field that does not.
    ///
    /// # Errors
    ///
    /// `steps`, `request`, `weight`, `thinkMs`, `users`, `duration`, `rps`, `errorRate`,
    /// `p95Ms`, `windowS`, `variables`.
    pub fn check(&self) -> Result<(), &'static str> {
        if self.steps.is_empty() || self.steps.len() > MAX_STEPS {
            return Err("steps");
        }
        for step in &self.steps {
            if step.request.trim().is_empty() {
                return Err("request");
            }
            if step.weight == 0 {
                return Err("weight");
            }
            if step.think_ms.min > step.think_ms.max {
                return Err("thinkMs");
            }
        }
        let profile = &self.profile;
        if profile.users == 0 {
            return Err("users");
        }
        if profile.duration_s() == 0 {
            return Err("duration");
        }
        if profile
            .rps
            .is_some_and(|rps| !rps.is_finite() || rps <= 0.0)
        {
            return Err("rps");
        }
        let abort = &self.abort;
        if abort
            .error_rate
            .is_some_and(|rate| !rate.is_finite() || !(0.0..=1.0).contains(&rate))
        {
            return Err("errorRate");
        }
        if abort.p95_ms.is_some_and(|ms| !ms.is_finite() || ms <= 0.0) {
            return Err("p95Ms");
        }
        if abort.window_s == 0 {
            return Err("windowS");
        }
        let bad_variable = self
            .variables
            .iter()
            .any(|(name, values)| values.is_empty() || !rb_api_model::vars::is_name(name));
        if bad_variable {
            return Err("variables");
        }
        Ok(())
    }

    /// User `user`'s values: the `user`th of each variable, round again.
    #[must_use]
    pub fn values_of(&self, user: u32) -> Vec<(String, String)> {
        self.variables
            .iter()
            .filter_map(|(name, values)| {
                let at = usize::try_from(user).ok()? % values.len().max(1);
                values.get(at).map(|value| (name.clone(), value.clone()))
            })
            .collect()
    }
}

/// `text` with the user's `values`, `{{$vu}}` and `{{$iteration}}` put in; every other
/// variable stays for the API client.
#[must_use]
pub fn fill(text: &str, values: &[(String, String)], user: u32, iteration: u64) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        out.push_str(&rest[..open]);
        let after = &rest[open + 2..];
        let Some(close) = after.find("}}") else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = after[..close].trim();
        let value = match name {
            "$vu" => Some(user.to_string()),
            "$iteration" => Some(iteration.to_string()),
            _ => values
                .iter()
                .find(|(known, _)| known == name)
                .map(|(_, value)| value.clone()),
        };
        match value {
            Some(value) => out.push_str(&value),
            None => out.push_str(&rest[open..open + 2 + close + 2]),
        }
        rest = &after[close + 2..];
    }
    out.push_str(rest);
    out
}

fn fill_json(value: &mut Value, values: &[(String, String)], user: u32, iteration: u64) {
    match value {
        Value::String(text) => *text = fill(text, values, user, iteration),
        Value::Array(items) => {
            for item in items {
                fill_json(item, values, user, iteration);
            }
        }
        Value::Object(fields) => {
            for field in fields.values_mut() {
                fill_json(field, values, user, iteration);
            }
        }
        _ => {}
    }
}

/// `request` for user `user` in round `iteration`: [`fill`] in every text (URL, headers, body,
/// auth); the request as it is when it does not turn into JSON and back.
#[must_use]
pub fn personalize(
    request: &ApiRequest,
    values: &[(String, String)],
    user: u32,
    iteration: u64,
) -> ApiRequest {
    let Ok(mut value) = serde_json::to_value(request) else {
        return request.clone();
    };
    fill_json(&mut value, values, user, iteration);
    serde_json::from_value(value).unwrap_or_else(|_| request.clone())
}

#[cfg(test)]
#[path = "scenario_tests.rs"]
mod tests;
