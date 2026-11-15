//! The layout modes' part of the shell snapshot (RB-080-04, `rb_ipc::ModeState`): the split's
//! divider, the strip's scroll, the minimap's content and the canvas board's labels, from the
//! spec in effect.

use rb_engine::{CellRect, ViewHost as _};
use rb_ipc::{MapRect, MinimapItem, MinimapState, ModeState, SplitState, StripState};

use super::canvas::{self, WorldRect};
use super::canvas_ops::{canvas_state, strip_of};
use super::horizontal;
use super::spec::{ModeKind, ModeSession};
use crate::host::{Host, LABEL_HEIGHT};

/// What the toolbar shows of the mode in effect; `None` in grid and focus mode.
pub fn mode_state(host: &Host, session: &ModeSession) -> Option<ModeState> {
    let spec = host.mode_spec()?;
    let area = host.grid_area().unwrap_or_default();
    let sizes = host.mode_sizes();
    let mut state = ModeState::default();
    match &spec.kind {
        ModeKind::Split {
            orientation, ratio, ..
        } => {
            state.split = Some(SplitState {
                ratio_permille: permille(*ratio),
                vertical: *orientation == rb_config::SplitOrientation::Vertical,
                step_permille: permille(session.settings.split_step),
            });
        }
        ModeKind::Horizontal { .. } => {
            let strip = strip_of(&spec)?;
            let last = horizontal::last_first(&sizes, area, &strip);
            state.strip = Some(StripState {
                first: count(strip.first.min(last)),
                last_first: count(last),
                count: count(sizes.len()),
            });
        }
        ModeKind::Canvas { .. } => {
            if session.minimap.unwrap_or(session.settings.minimap) {
                state.canvas = Some(minimap(host, &spec, area, session));
            }
            state.board = Some(super::canvas_board::board_state(host, &spec, area, session));
        }
    }
    Some(state)
}

fn minimap(
    host: &Host,
    spec: &super::ModeSpec,
    area: CellRect,
    session: &ModeSession,
) -> MinimapState {
    let sizes = host.mode_sizes();
    let (items, camera) = canvas_state(spec, &sizes, area);
    let slots = spec.canvas_slots(&items, &sizes, area, camera, LABEL_HEIGHT);
    let order = host.views();
    let views: Vec<_> = items
        .iter()
        .zip(&sizes)
        .zip(&slots)
        .filter_map(|((item, size), slot)| {
            let view = order
                .iter()
                .find(|info| info.cell_breakpoint.as_deref() == Some(item.breakpoint.as_str()))?
                .id;
            Some(MinimapItem {
                view,
                breakpoint: item.breakpoint.clone(),
                rect: map_rect(canvas::outer(item, size, LABEL_HEIGHT)),
                visible: slot.visible,
            })
        })
        .collect();
    MinimapState {
        position: session.settings.minimap_position,
        items: views,
        viewport: map_rect(canvas::viewport(area, camera)),
    }
}

fn map_rect(rect: WorldRect) -> MapRect {
    let whole = |value: f64| {
        if value.is_finite() {
            // Held to the i32 range first.
            value
                .round()
                .clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        } else {
            0
        }
    };
    MapRect {
        x: whole(rect.x),
        y: whole(rect.y),
        width: u32::try_from(whole(rect.width)).unwrap_or(0),
        height: u32::try_from(whole(rect.height)).unwrap_or(0),
    }
}

fn permille(share: f64) -> u32 {
    super::to_dip(share * 1000.0)
}

fn count(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

#[cfg(test)]
#[path = "ipc_tests.rs"]
mod tests;
