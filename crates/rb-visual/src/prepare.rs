//! The capture preparation (RB-080-12): before every capture - a view, full-page or element
//! screenshot, the overview, a baseline, a sitemap shot, an environment comparison - the page's
//! noise goes and its motion stops, and afterwards everything is put back. This module is the
//! browser-free half: the built-in selector list (`data/hide-selectors.json`, consent managers
//! and chat widgets), the settings of Capture > Cookie banners & animations ([`CleanSettings`]),
//! what one capture does ([`CleanPlan`], the settings with the call's [`CleanOverrides`] over
//! them) and the page script it runs in rBrowser's isolated world ([`WORLD`],
//! `prepare/clean.js`). The CDP side - the world, `Animation.setPlaybackRate(0)` and
//! `prefers-reduced-motion: reduce` for the capture - runs in the app
//! (`crates/rbrowser/src/capture/clean.rs`).

use std::sync::LazyLock;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

/// The isolated world the preparation runs in (`Page.createIsolatedWorld`).
pub const WORLD: &str = "rbrowser-capture";
/// `capture.settleMs` when not set: the wait after the preparation, in milliseconds.
pub const DEFAULT_SETTLE_MS: u32 = 100;
/// The largest `capture.settleMs`.
pub const MAX_SETTLE_MS: u32 = 5000;
/// The most selectors one capture hides (the built-in list and the project's).
pub const MAX_SELECTORS: usize = 320;
/// The most selectors `capture.hideSelectors` takes.
pub const MAX_PROJECT_SELECTORS: usize = 200;
/// The longest selector taken.
pub const MAX_SELECTOR_LEN: usize = 1024;
/// The licences a source of the built-in list may carry: rBrowser's own, MIT or CC0.
pub const ALLOWED_LICENCES: [&str; 3] = ["GPL-3.0-or-later", "MIT", "CC0-1.0"];

/// The page script: defines `__rbCaptureClean` in the world once.
const CLEAN_JS: &str = include_str!("prepare/clean.js");
/// The built-in list, as shipped.
pub const BUILTIN_JSON: &str = include_str!("../data/hide-selectors.json");

/// The built-in list's file.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BuiltinList {
    /// The file format, `1`.
    pub version: u32,
    /// Where the selectors come from, each with its licence.
    pub sources: Vec<Source>,
    /// One group per consent manager or chat widget.
    pub groups: Vec<Group>,
}

/// A source of the built-in list.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Its id, named by the groups.
    pub id: String,
    /// What it is.
    pub name: String,
    /// Its SPDX licence: one of [`ALLOWED_LICENCES`].
    pub licence: String,
}

/// One consent manager or chat widget.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Group {
    /// Its id.
    pub id: String,
    /// The product's name.
    pub name: String,
    /// `consent` or `chat`.
    pub kind: String,
    /// The source's id.
    pub source: String,
    /// Its CSS selectors.
    pub selectors: Vec<String>,
}

/// The built-in selectors in file order; empty if the shipped file were broken (a test holds it
/// sound).
pub static BUILTIN: LazyLock<Vec<String>> = LazyLock::new(|| {
    serde_json::from_str::<BuiltinList>(BUILTIN_JSON)
        .map(|list| {
            list.groups
                .into_iter()
                .flat_map(|group| group.selectors)
                .collect()
        })
        .unwrap_or_default()
});

/// Capture > Cookie banners & animations: the `capture` keys of this job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanSettings {
    /// `capture.hideBanners`.
    pub hide_banners: bool,
    /// `capture.hideSelectors`: the project's selectors, beside the built-in list.
    pub hide_selectors: Vec<String>,
    /// `capture.useBuiltinList`.
    pub use_builtin_list: bool,
    /// `capture.freezeAnimations`.
    pub freeze_animations: bool,
    /// `capture.settleMs`.
    pub settle_ms: u32,
}

impl Default for CleanSettings {
    fn default() -> Self {
        Self {
            hide_banners: true,
            hide_selectors: Vec::new(),
            use_builtin_list: true,
            freeze_animations: true,
            settle_ms: DEFAULT_SETTLE_MS,
        }
    }
}

/// A call's (or the session's) word over the settings; `None` leaves the setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CleanOverrides {
    /// Hide the banners.
    pub hide_banners: Option<bool>,
    /// Freeze the animations.
    pub freeze_animations: Option<bool>,
}

impl CleanOverrides {
    /// These overrides, with what they leave out taken from `fallback`.
    #[must_use]
    pub fn or(self, fallback: CleanOverrides) -> CleanOverrides {
        CleanOverrides {
            hide_banners: self.hide_banners.or(fallback.hide_banners),
            freeze_animations: self.freeze_animations.or(fallback.freeze_animations),
        }
    }
}

/// What one capture does to the page.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CleanPlan {
    /// Hide `selectors`.
    pub hide: bool,
    /// The selectors, bounded and without duplicates.
    pub selectors: Vec<String>,
    /// Freeze animations and videos, `prefers-reduced-motion: reduce`.
    pub freeze: bool,
    /// Black out the password fields (a bug report's capture, RB-093-04); no settings key, the
    /// caller sets it.
    pub mask: bool,
    /// The wait after the preparation, in milliseconds.
    pub settle_ms: u32,
}

/// `selector` trimmed, when it is one to take.
fn usable(selector: &str) -> Option<&str> {
    let selector = selector.trim();
    (!selector.is_empty() && selector.len() <= MAX_SELECTOR_LEN).then_some(selector)
}

/// The list `capture.hideSelectors` takes from `value`: strings of 1 to [`MAX_SELECTOR_LEN`]
/// characters, at most [`MAX_PROJECT_SELECTORS`].
pub fn parse_selectors(value: &Value) -> Result<Vec<String>, String> {
    let items = value.as_array().ok_or("not a list")?;
    if items.len() > MAX_PROJECT_SELECTORS {
        return Err(format!("more than {MAX_PROJECT_SELECTORS} selectors"));
    }
    items
        .iter()
        .map(|item| {
            item.as_str()
                .and_then(usable)
                .map(str::to_owned)
                .ok_or_else(|| format!("a selector is 1 to {MAX_SELECTOR_LEN} characters"))
        })
        .collect()
}

impl CleanSettings {
    /// The plan of one capture: `overrides` over these settings; `extra` are selectors of an
    /// older key that still count (`sitemap.hideSelectors`).
    pub fn plan(&self, extra: &[String], overrides: CleanOverrides) -> CleanPlan {
        let hide = overrides.hide_banners.unwrap_or(self.hide_banners);
        let freeze = overrides
            .freeze_animations
            .unwrap_or(self.freeze_animations);
        let mut selectors: Vec<String> = Vec::new();
        if hide {
            let builtin = self
                .use_builtin_list
                .then(|| BUILTIN.iter())
                .into_iter()
                .flatten();
            for selector in builtin.chain(&self.hide_selectors).chain(extra) {
                let Some(selector) = usable(selector) else {
                    continue;
                };
                if selectors.len() >= MAX_SELECTORS {
                    break;
                }
                if !selectors.iter().any(|known| known == selector) {
                    selectors.push(selector.to_owned());
                }
            }
        }
        CleanPlan {
            hide: hide && !selectors.is_empty(),
            selectors,
            freeze,
            mask: false,
            settle_ms: self.settle_ms.min(MAX_SETTLE_MS),
        }
    }
}

impl CleanPlan {
    /// Whether the plan leaves the page alone.
    pub fn is_noop(&self) -> bool {
        !self.hide && !self.freeze && !self.mask
    }

    /// The wait after the preparation.
    pub fn settle(&self) -> Duration {
        if self.is_noop() {
            Duration::ZERO
        } else {
            Duration::from_millis(u64::from(self.settle_ms))
        }
    }

    /// The expression that prepares the page in [`WORLD`]; it answers
    /// `{ token, rules, matched, animations, videos }`.
    pub fn prepare_expression(&self) -> String {
        let options = json!({
            "hide": self.hide,
            "selectors": self.selectors,
            "freeze": self.freeze,
            "mask": self.mask,
        });
        format!("{CLEAN_JS}\nglobalThis.__rbCaptureClean.prepare({options})")
    }
}

/// The expression that puts the page back after the preparation `token`.
pub fn undo_expression(token: u64) -> String {
    format!("{CLEAN_JS}\nglobalThis.__rbCaptureClean.undo({token})")
}

/// `media` (the parameters of `Emulation.setEmulatedMedia` the view has) with
/// `prefers-reduced-motion: reduce`, every other feature kept.
pub fn reduced_motion_media(media: &Value) -> Value {
    let mut features: Vec<Value> = media
        .get("features")
        .and_then(Value::as_array)
        .map(|features| {
            features
                .iter()
                .filter(|feature| {
                    feature.get("name").and_then(Value::as_str) != Some("prefers-reduced-motion")
                })
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    features.push(json!({ "name": "prefers-reduced-motion", "value": "reduce" }));
    json!({
        "media": media.get("media").and_then(Value::as_str).unwrap_or(""),
        "features": features,
    })
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
