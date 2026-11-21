//! The plugin's settings (`settings.schema.json`, Settings > Checks > Width sweep): the first
//! and last width, the step and the kinds of break to look for; a call's arguments (`from`,
//! `to`, `step`, `kinds`) win over them. And how long "Show at N px" holds a width
//! (`showSeconds`, [`show_seconds`]). A stored value out of range keeps the default; an
//! argument out of range is refused.

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::Value;

pub use crate::plan::Span;

/// The narrowest width, CSS px: the host's lower bound of a plugin's width
/// (`rb_plugin_host` `MIN_PLUGIN_WIDTH`).
pub const MIN_WIDTH: u32 = 120;
/// The widest, CSS px (`MAX_PLUGIN_WIDTH`).
pub const MAX_WIDTH: u32 = 3840;
/// The largest step, CSS px.
pub const MAX_STEP: u32 = 400;
/// How long "Show at N px" holds a width by default, seconds (`showSeconds`).
pub const SHOW_SECONDS: u32 = 120;
/// The shortest hold `showSeconds` may name.
pub const MIN_SHOW_SECONDS: u32 = 10;
/// The longest: below the plugin's call timeout of 300 s, so the hold ends before the call would.
pub const MAX_SHOW_SECONDS: u32 = 240;
/// The kinds of break, in the order the findings name them.
pub const KINDS: [&str; 3] = ["overflow", "text-clipped", "overlap"];

/// What a sweep runs with.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub from: u32,
    pub to: u32,
    pub step: u32,
    /// Known kinds, each once, in the order of [`KINDS`].
    pub kinds: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            from: 320,
            to: 1920,
            step: 10,
            kinds: KINDS.iter().map(|kind| (*kind).to_owned()).collect(),
        }
    }
}

fn invalid(argument: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("argument", argument)
}

/// A width in range, [`MIN_WIDTH`] to [`MAX_WIDTH`].
pub(crate) fn width(value: &Value) -> Option<u32> {
    let width = u32::try_from(value.as_u64()?).ok()?;
    (MIN_WIDTH..=MAX_WIDTH).contains(&width).then_some(width)
}

fn step(value: &Value) -> Option<u32> {
    let step = u32::try_from(value.as_u64()?).ok()?;
    (1..=MAX_STEP).contains(&step).then_some(step)
}

/// The known kinds `value` lists, each once, in the order of [`KINDS`]; `None` for no list or
/// one without a known kind.
fn kinds(value: &Value) -> Option<Vec<String>> {
    let listed = value.as_array()?;
    let known: Vec<String> = KINDS
        .iter()
        .filter(|kind| listed.iter().any(|item| item.as_str() == Some(**kind)))
        .map(|kind| (*kind).to_owned())
        .collect();
    (!known.is_empty()).then_some(known)
}

impl Settings {
    /// The values in effect; each missing or out of range keeps its default.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        let mut setting = |key: &str| {
            host.setting(key)
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
                .unwrap_or(Value::Null)
        };
        let defaults = Self::default();
        Self {
            from: width(&setting("from")).unwrap_or(defaults.from),
            to: width(&setting("to")).unwrap_or(defaults.to),
            step: step(&setting("step")).unwrap_or(defaults.step),
            kinds: kinds(&setting("kinds")).unwrap_or(defaults.kinds),
        }
    }

    /// These settings with the call's `args` over them.
    ///
    /// # Errors
    ///
    /// `plugins.error.invalid` naming the argument out of range, of the wrong type, or (`kinds`)
    /// naming an unknown kind or none.
    pub fn with_args(mut self, args: &Value) -> Result<Self, Failure> {
        if let Some(value) = args.get("from") {
            self.from = width(value).ok_or_else(|| invalid("from"))?;
        }
        if let Some(value) = args.get("to") {
            self.to = width(value).ok_or_else(|| invalid("to"))?;
        }
        if let Some(value) = args.get("step") {
            self.step = step(value).ok_or_else(|| invalid("step"))?;
        }
        if let Some(value) = args.get("kinds") {
            let listed = value.as_array().ok_or_else(|| invalid("kinds"))?;
            let all_known = listed
                .iter()
                .all(|item| item.as_str().is_some_and(|kind| KINDS.contains(&kind)));
            if !all_known {
                return Err(invalid("kinds"));
            }
            self.kinds = kinds(value).ok_or_else(|| invalid("kinds"))?;
        }
        Ok(self)
    }

    /// The widths to sweep.
    ///
    /// # Errors
    ///
    /// `plugins.error.invalid` naming `to` when the first width passes the last.
    pub fn span(&self) -> Result<Span, Failure> {
        if self.from > self.to {
            return Err(invalid("to"));
        }
        Ok(Span {
            from: self.from,
            to: self.to,
            step: self.step,
        })
    }
}

/// The setting `showSeconds` in effect; missing or out of range, [`SHOW_SECONDS`].
pub fn show_seconds(host: &mut dyn PluginHost) -> u32 {
    host.setting("showSeconds")
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|value| u32::try_from(value.as_u64()?).ok())
        .filter(|seconds| (MIN_SHOW_SECONDS..=MAX_SHOW_SECONDS).contains(seconds))
        .unwrap_or(SHOW_SECONDS)
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
