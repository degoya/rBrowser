//! The MCP tool `screenshot` (RB-050-03, extending RB-030-01's core tool): its name, its
//! description, its arguments and its answer. Until the MCP server exists (RB-030-01) the
//! commands `capture.*` take the same arguments; the server registers the tool with these
//! types and calls [`super::screenshot`].

use std::path::PathBuf;
use std::time::Duration;

use rb_engine::{Target, ViewId};
use rb_visual::CleanOverrides;
use rb_visual::delay::{MAX_DELAY_MS, clamp_delay_ms};
use rb_visual::settings::MAX_PADDING;
use serde::{Deserialize, Serialize};

use super::cdp::MAX_SELECTOR_LEN;

/// The tool's name (a core tool: no prefix).
pub const TOOL_NAME: &str = "screenshot";
/// The tool's description for the agent.
pub const TOOL_DESCRIPTION: &str = "Capture views as PNG: the viewport at the breakpoint's \
size, the full page, or one element (CSS selector, with padding), from one view or all \
(target \"all\" or a view id; the active view when absent). overview: one image of all views \
side by side with label and size, views out of sight included. delayMs waits first (at most \
60000). save (default true) writes the files under the project's screenshot folder and answers \
their paths; save false answers the PNGs as base64 instead. hideBanners and freezeAnimations \
override capture.hideBanners and capture.freezeAnimations for this call.";
/// The most bytes of PNG an answer carries inline (`save: false`).
pub const MAX_INLINE_BYTES: usize = 16 * 1024 * 1024;

/// What is captured of each view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    /// The viewport.
    #[default]
    Viewport,
    /// The whole page.
    FullPage,
    /// One element (`selector`).
    Element,
}

impl CaptureMode {
    /// The `{mode}` of the path pattern.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Viewport => "viewport",
            Self::FullPage => "fullPage",
            Self::Element => "element",
        }
    }
}

/// The arguments of the tool and of the `capture.*` commands; every field may be left out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScreenshotArgs {
    /// `"all"` or a view id; the active view when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// `viewport` (default), `fullPage` or `element`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<CaptureMode>,
    /// The element's CSS selector (`mode: element`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selector: Option<String>,
    /// CSS px around the element; `capture.elementPadding` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub padding: Option<u32>,
    /// One overview image of the views instead of a file each.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<bool>,
    /// Wait this long first (at most 60000).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u64>,
    /// Write files (default) or answer base64.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub save: Option<bool>,
    /// Hide cookie banners and chat widgets; `capture.hideBanners` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hide_banners: Option<bool>,
    /// Freeze animations and videos; `capture.freezeAnimations` when absent (RB-080-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freeze_animations: Option<bool>,
    /// The element at this point of the view (`mode: element`, no selector): the context
    /// menu's "Element under the cursor" and MCP's `at` (RB-080-43).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<PointArg>,
}

/// A point of a view in whole pixels: CSS px of the page (MCP), or with `dip` the DIP of the
/// view's widget as the context menu's click gives them, divided by the cell's scale first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointArg {
    pub x: i32,
    pub y: i32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub dip: bool,
}

impl PointArg {
    fn usable(self) -> bool {
        self.x >= 0 && self.y >= 0
    }
}

impl ScreenshotArgs {
    /// These arguments, with what they leave out taken from `defaults` (a command's preset).
    pub fn or(self, defaults: &ScreenshotArgs) -> ScreenshotArgs {
        ScreenshotArgs {
            target: self.target.or(defaults.target),
            mode: self.mode.or(defaults.mode),
            selector: self.selector.or_else(|| defaults.selector.clone()),
            padding: self.padding.or(defaults.padding),
            overview: self.overview.or(defaults.overview),
            delay_ms: self.delay_ms.or(defaults.delay_ms),
            save: self.save.or(defaults.save),
            hide_banners: self.hide_banners.or(defaults.hide_banners),
            freeze_animations: self.freeze_animations.or(defaults.freeze_animations),
            at: self.at.or(defaults.at),
        }
    }

    /// The checked request. `padding` and `delay` are the defaults from the settings and the
    /// delay cycle; `pick` lets an element capture without a selector ask the user to click one
    /// (the commands; never MCP, where nobody clicks).
    pub fn request(
        &self,
        padding: u32,
        delay: Duration,
        pick: bool,
    ) -> Result<ScreenshotRequest, String> {
        let mode = self.mode.unwrap_or_default();
        let overview = self.overview.unwrap_or(false);
        let padding = self.padding.unwrap_or(padding);
        if padding > MAX_PADDING {
            return Err(format!("padding is at most {MAX_PADDING}"));
        }
        if self.delay_ms.is_some_and(|delay| delay > MAX_DELAY_MS) {
            return Err(format!("delayMs is at most {MAX_DELAY_MS}"));
        }
        let selector = match (mode, &self.selector) {
            (CaptureMode::Element, Some(selector)) => {
                let selector = selector.trim();
                if selector.is_empty() || selector.len() > MAX_SELECTOR_LEN {
                    return Err(format!(
                        "selector must have 1 to {MAX_SELECTOR_LEN} characters"
                    ));
                }
                Some(selector.to_owned())
            }
            (CaptureMode::Element, None) if pick || self.at.is_some() => None,
            (CaptureMode::Element, None) => return Err("mode element needs a selector".to_owned()),
            (_, Some(_)) => return Err("selector is only for mode element".to_owned()),
            (_, None) => None,
        };
        if overview && mode == CaptureMode::Element {
            return Err("the overview takes mode viewport or fullPage".to_owned());
        }
        if selector.is_none() && mode == CaptureMode::Element && self.target == Some(Target::All) {
            return Err("picking an element works in one view".to_owned());
        }
        match self.at {
            Some(_) if mode != CaptureMode::Element || selector.is_some() => {
                return Err("`at` goes with mode element and no selector".to_owned());
            }
            Some(point) if !point.usable() => {
                return Err("`at` is a point of non-negative coordinates".to_owned());
            }
            _ => {}
        }
        Ok(ScreenshotRequest {
            target: self.target,
            mode,
            selector,
            padding,
            overview,
            delay: self.delay_ms.map_or(delay, clamp_delay_ms),
            save: self.save.unwrap_or(true),
            views: None,
            folder: None,
            clean: CleanOverrides {
                hide_banners: self.hide_banners,
                freeze_animations: self.freeze_animations,
            },
            at: self.at,
        })
    }
}

/// A checked [`ScreenshotArgs`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScreenshotRequest {
    /// The views; the active one when `None`.
    pub target: Option<Target>,
    /// What of each view.
    pub mode: CaptureMode,
    /// The element; `None` in mode element: the user picks it.
    pub selector: Option<String>,
    /// CSS px around an element.
    pub padding: u32,
    /// One overview image.
    pub overview: bool,
    /// The wait before.
    pub delay: Duration,
    /// Files, or base64 in the answer.
    pub save: bool,
    /// These views instead of `target` (the CLI's `--views`), in layout order.
    pub views: Option<Vec<ViewId>>,
    /// This folder instead of `capture.dir` (the CLI's `--out`, the user's own choice).
    pub folder: Option<PathBuf>,
    /// The call's word on hiding banners and freezing animations (RB-080-12).
    pub clean: CleanOverrides,
    /// The element at this point of the one view (mode element without a selector): named by
    /// the context menu, so nobody picks (RB-080-43).
    pub at: Option<PointArg>,
}

/// One image of the answer.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShotAnswer {
    /// The view; `None` for the overview.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<ViewId>,
    /// The view's label (`overview` for the overview).
    pub label: String,
    /// The image's width in pixels (`0` after an error).
    pub width: u32,
    /// The image's height in pixels.
    pub height: u32,
    /// The file written (`save: true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// The PNG as base64 (`save: false`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub png_base64: Option<String>,
    /// Why this view gave no image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The answer of the tool and the commands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotAnswer {
    /// One entry per view, in layout order (with `overview` without file or data).
    pub shots: Vec<ShotAnswer>,
    /// The overview image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overview: Option<ShotAnswer>,
    /// The folder of the files (`save: true`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<String>,
}

impl ScreenshotAnswer {
    /// Every file written, the overview last.
    pub fn paths(&self) -> Vec<&str> {
        self.shots
            .iter()
            .chain(&self.overview)
            .filter_map(|shot| shot.path.as_deref())
            .collect()
    }
}

#[cfg(test)]
#[path = "tool_tests.rs"]
mod tests;
