//! The command and tool `show` (`plugin.width-sweep.show`, MCP `sweep_show`, RB-093-01): one
//! width held for a person to look at it - what "Show at N px" on a width sweep finding runs
//! with the finding's view and width (`data.width`, the middle of its range).
//!
//! The width is the host's width lease, as a sweep's: the view leaves the sync while it is held
//! and gets its own width and sync back at the end. The call holds it in the page
//! (`WidthSweepDetect.hold`, `web/src/hold.ts`) until the first of Escape in the view, the
//! setting `showSeconds`, or the view leaving the width - the `cancel` command ended the lease,
//! or another `show` or a sweep set another width; in that last case the lease is not this
//! call's any more and is left alone. However the call ends, the host ends what is left of the
//! lease with the plugin's last running call. While it holds, it reports progress without a
//! total, so the activity bar shows the plugin with its Cancel.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewId, codes};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::probe::{CLEAR, DETECT, PAGE_FAILED_KEY, SET};
use crate::settings;
use crate::tool;

/// How a hold ended.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum End {
    /// Escape was pressed in the view.
    Escape,
    /// The view left the width: the lease is no longer this call's.
    Resized,
    /// The time given went by.
    Elapsed,
}

impl End {
    fn name(self) -> &'static str {
        match self {
            Self::Escape => "escape",
            Self::Resized => "resized",
            Self::Elapsed => "elapsed",
        }
    }
}

#[derive(Deserialize)]
struct Held {
    end: End,
}

/// The expression `evaluate` runs to hold `width` for at most `ms` milliseconds.
#[must_use]
pub fn script(width: u32, ms: u64) -> String {
    let options = json!({ "width": width, "ms": ms });
    format!("{DETECT}\n;WidthSweepDetect.hold({options})")
}

fn hold(host: &mut dyn PluginHost, view: ViewId, width: u32, ms: u64) -> Result<End, Failure> {
    let text = host.evaluate(view, &script(width, ms))?;
    serde_json::from_str::<Held>(&text)
        .map(|held| held.end)
        .map_err(|_| Failure::coded(FailureKind::BadData, PAGE_FAILED_KEY))
}

/// `show`: holds `width` on the view `target` names (the active view without one); the answer
/// names the view, the width and how the hold ended.
///
/// # Errors
///
/// A `width` missing or out of range, a view that is gone or runs another engine, the width
/// refused by the host (another plugin leases the view), or the page's failure.
pub fn show(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let width = args.get("width").and_then(settings::width).ok_or_else(|| {
        Failure::coded(FailureKind::BadData, codes::INVALID).with_param("argument", "width")
    })?;
    let view = tool::target(host, args.get("target"))?;
    view.require_chromium("cdp")?;
    let ms = u64::from(settings::show_seconds(host)) * 1000;
    host.cdp_send(view.id, SET, &json!({ "width": width }).to_string())?;
    host.report_progress(0, 0);
    let held = hold(host, view.id, width, ms);
    if !matches!(held, Ok(End::Resized))
        && let Err(failure) = host.cdp_send(view.id, CLEAR, "{}")
    {
        host.log("debug", &format!("the width lease did not end: {failure}"));
    }
    let end = held?;
    Ok(json!({ "view": view.id, "width": width, "end": end.name() }))
}

#[cfg(test)]
#[path = "show_tests.rs"]
mod tests;
