//! What the plugin reads from the project's `.rbrowser/config.json` (capability `project-read`):
//! `checks.lighthouse.categories` - the project's key from the plan, in the config schema and
//! the settings window under Checks > Lighthouse - and per breakpoint the emulation the views do
//! not report (`touch`, `mobile`, `userAgent`). The host never lets a plugin read
//! `config.local.json`, so a local override of these keys does not reach Lighthouse. A config
//! that cannot be read leaves the defaults (all four categories, mobile from the width).

use std::collections::BTreeMap;

use rb_plugin_api::PluginHost;
use serde_json::Value;

use crate::rules;

/// The project's config, relative to the project's root.
pub const CONFIG_PATH: &str = ".rbrowser/config.json";

/// What a breakpoint adds to its view.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BreakpointExtras {
    pub touch: Option<bool>,
    pub mobile: Option<bool>,
    pub user_agent: Option<String>,
}

/// The project's Lighthouse settings and breakpoints.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Project {
    /// `checks.lighthouse.categories`, the known ones only; empty for all.
    pub categories: Vec<String>,
    /// By breakpoint id.
    pub breakpoints: BTreeMap<String, BreakpointExtras>,
}

impl Project {
    /// Reads the project's config text.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let Ok(config) = serde_json::from_str::<Value>(text) else {
            return Self::default();
        };
        let categories = config["checks"]["lighthouse"]["categories"]
            .as_array()
            .map(|list| {
                list.iter()
                    .filter_map(Value::as_str)
                    .filter(|category| rules::is_category(category))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let mut breakpoints = BTreeMap::new();
        for breakpoint in config["breakpoints"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        {
            let Some(id) = breakpoint["id"].as_str() else {
                continue;
            };
            breakpoints.insert(
                id.to_owned(),
                BreakpointExtras {
                    touch: breakpoint["touch"].as_bool(),
                    mobile: breakpoint["mobile"].as_bool(),
                    user_agent: breakpoint["userAgent"]
                        .as_str()
                        .filter(|agent| !agent.is_empty())
                        .map(str::to_owned),
                },
            );
        }
        Self {
            categories,
            breakpoints,
        }
    }

    /// Reads `.rbrowser/config.json`; the defaults when it cannot be read.
    pub fn read(host: &mut dyn PluginHost) -> Self {
        match host.project_read(CONFIG_PATH) {
            Ok(bytes) => Self::parse(&String::from_utf8_lossy(&bytes)),
            Err(failure) => {
                host.log("info", &format!("{CONFIG_PATH} not read: {failure}"));
                Self::default()
            }
        }
    }
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
