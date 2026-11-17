//! The arguments of the MCP tool `autofill_fill` and the commands: `mode`, `selector`,
//! `profile`, `seed`, `strategy` and `target` (`"all"` or a view id; the host resolves a
//! breakpoint to one call per view). Every other key - the host's `$`-keys included - is ignored.

use rb_plugin_api::{Failure, FailureKind, ViewId, codes};
use serde_json::{Map, Value};

use crate::invalid::Strategy;

/// The longest selector taken.
pub const MAX_SELECTOR: usize = 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    Valid,
    Invalid,
}

impl Mode {
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Mode::Valid => "valid",
            Mode::Invalid => "invalid",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Target {
    All,
    View(ViewId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Args {
    pub mode: Option<Mode>,
    pub selector: Option<String>,
    pub profile: Option<String>,
    pub seed: Option<u64>,
    pub strategy: Option<Strategy>,
    pub target: Target,
}

/// `plugins.error.invalid` naming the argument.
#[must_use]
pub fn bad(reason: &str) -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", reason)
}

fn text(map: &Map<String, Value>, key: &str, max: usize) -> Result<Option<String>, Failure> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.chars().count() <= max => {
            Ok(Some(text.trim().to_owned()).filter(|text| !text.is_empty()))
        }
        Some(_) => Err(bad(key)),
    }
}

impl Args {
    /// # Errors
    ///
    /// `plugins.error.invalid` with `reason` naming the argument that does not read.
    pub fn parse(args: &str) -> Result<Self, Failure> {
        let value: Value = if args.trim().is_empty() {
            Value::Null
        } else {
            serde_json::from_str(args).map_err(|_| bad("args"))?
        };
        let empty = Map::new();
        let map = match &value {
            Value::Object(map) => map,
            Value::Null => &empty,
            _ => return Err(bad("args")),
        };
        let mode = match map.get("mode").and_then(Value::as_str) {
            None if map.get("mode").is_none_or(Value::is_null) => None,
            Some("valid") => Some(Mode::Valid),
            Some("invalid") => Some(Mode::Invalid),
            _ => return Err(bad("mode")),
        };
        let profile = match map.get("profile") {
            Some(Value::String(name)) if name.trim().is_empty() => return Err(bad("profile")),
            _ => text(map, "profile", 80)?,
        };
        let seed = match map.get("seed") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_u64()
                    .filter(|seed| *seed <= u64::from(u32::MAX))
                    .ok_or_else(|| bad("seed"))?,
            ),
        };
        let strategy = match map.get("strategy") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_str()
                    .and_then(Strategy::parse)
                    .ok_or_else(|| bad("strategy"))?,
            ),
        };
        let target = match map.get("target") {
            None | Some(Value::Null) => Target::All,
            Some(Value::String(all)) if all == "all" => Target::All,
            Some(value) => Target::View(
                value
                    .as_u64()
                    .and_then(|id| ViewId::try_from(id).ok())
                    .ok_or_else(|| bad("target"))?,
            ),
        };
        Ok(Self {
            mode,
            selector: text(map, "selector", MAX_SELECTOR)?,
            profile,
            seed,
            strategy,
            target,
        })
    }
}

#[cfg(test)]
#[path = "args_tests.rs"]
mod tests;
