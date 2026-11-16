//! What the breakpoint shortcuts do to a view (RB-010-16): set it to a breakpoint or a free
//! size through the host - live emulation, no reload, sync stays on (RB-010-05) - or open a new
//! view with it (the new-view modifier), then show the overlay in the cell's label strip.

use std::time::Instant;

use rb_commands::CommandError;
use rb_config::{Breakpoint, DevicePreset};
use rb_engine::{EngineError, Target, ViewHost, ViewId};
use serde_json::{Value, json};

use super::sizes::{self, FreeSize};
use crate::control::Core;
use crate::emulation::device_list;
use crate::host::ui::CellOverlay;

/// The i18n key of the overlay text; placeholders `{label}`, `{width}`, `{height}`, `{dpr}`.
pub const OVERLAY_KEY: &str = "sizes.overlay";

/// Where an action goes: a view (the active one when `None`), or a new view beside it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Goal {
    /// The view; the active view (else the first) when `None`.
    pub view: Option<ViewId>,
    /// Open a new view with the size instead; the view stays as it is.
    pub new_view: bool,
}

/// What a size change becomes.
pub enum Change<'a> {
    /// A breakpoint: its size and its full emulation.
    Breakpoint(&'a Breakpoint),
    /// A free size: the rest of the emulation stays.
    Size(FreeSize),
}

/// The view `goal` names: the given one, else the active view, else the first.
pub fn view_of(core: &Core, goal: Goal) -> Result<ViewId, CommandError> {
    core.host()
        .target_views(goal.view.map(Target::View))
        .map_err(failed)?
        .first()
        .copied()
        .ok_or_else(|| CommandError::Failed("there is no view".to_owned()))
}

/// Applies `change` to the view of `goal` (or a new view) and shows the overlay; answers
/// `{ view, width, height, dpr, newView, elapsedMs }`. The time is the core's part, from the
/// command to the emulation handed to CDP; the page follows within the same frame.
pub fn apply(core: &Core, goal: Goal, change: Change<'_>) -> Result<Value, CommandError> {
    let started = Instant::now();
    let host = core.host();
    let source = view_of(core, goal)?;
    let view = if goal.new_view {
        let breakpoint = match change {
            Change::Breakpoint(breakpoint) => breakpoint.clone(),
            Change::Size(size) => size.breakpoint(),
        };
        let url = host.view(source).map(|info| info.url);
        let view = host.open_view(&breakpoint).map_err(failed)?;
        if let Some(url) = url.filter(|url| is_web_url(url)) {
            host.navigate(Target::View(view), &url).map_err(failed)?;
        }
        view
    } else {
        match change {
            Change::Breakpoint(breakpoint) => host.set_breakpoint(source, breakpoint),
            Change::Size(size) => host.set_size(source, size.width, size.height, size.dpr),
        }
        .map_err(failed)?;
        source
    };
    let elapsed = started.elapsed();
    tracing::debug!(%view, elapsed_ms = elapsed.as_secs_f64() * 1000.0, "breakpoint applied");
    show_overlay(core, view);
    let emulation = host.emulation(view).unwrap_or_default();
    Ok(json!({
        "view": view,
        "width": emulation.width,
        "height": emulation.height,
        "dpr": emulation.dpr,
        "newView": goal.new_view,
        "elapsedMs": elapsed.as_secs_f64() * 1000.0,
    }))
}

/// Writes the view's new label into its label strip, first the overlay for
/// `breakpointShortcuts.overlayMs` (none at 0).
pub fn show_overlay(core: &Core, view: ViewId) {
    let host = core.host();
    let Some(info) = host.view(view) else {
        return;
    };
    let emulation = host.emulation(view).unwrap_or_default();
    let ms = core.user_settings().breakpoint_shortcuts.overlay_ms;
    let overlay = (ms > 0).then(|| CellOverlay {
        text: overlay_text(
            &info.label,
            emulation.width,
            emulation.height,
            emulation.dpr,
        ),
        ms,
    });
    host.ui().set_cell_label(view, info.label, overlay);
}

/// `Tablet - 768 x 1024 @2x` in the catalogue's words.
pub fn overlay_text(label: &str, width: u32, height: u32, dpr: f64) -> String {
    let dpr = format_dpr(dpr);
    rb_i18n::t_with(
        OVERLAY_KEY,
        &[
            ("label", label),
            ("width", &width.to_string()),
            ("height", &height.to_string()),
            ("dpr", &dpr),
        ],
    )
}

/// A DPR with at most three decimals and no trailing zeros (`2`, `2.625`).
pub fn format_dpr(dpr: f64) -> String {
    let rounded = (dpr * 1000.0).round() / 1000.0;
    let text = format!("{rounded:.3}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// The display name of a device preset: its own label, else the catalogue's `devices.<id>`.
pub fn device_label(device: &DevicePreset) -> String {
    device
        .label
        .clone()
        .unwrap_or_else(|| rb_i18n::t(&format!("devices.{}", device.id)))
}

/// The breakpoint `id` names: the config's, a device preset (RB-010-12) or a free size.
pub fn find(core: &Core, id: &str) -> Result<Breakpoint, CommandError> {
    sizes::find_breakpoint(&core.host().config(), &device_list(), id, device_label)
        .ok_or_else(|| CommandError::InvalidArgs(format!("no breakpoint or device `{id}`")))
}

/// Whether a page at `url` is one a new view may load (never `about:` or the app's scheme).
fn is_web_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// A host error as a command error.
pub fn failed(error: EngineError) -> CommandError {
    match error {
        EngineError::UnknownView(_) | EngineError::Invalid(_) => {
            CommandError::InvalidArgs(error.to_string())
        }
        other => CommandError::Failed(other.to_string()),
    }
}
