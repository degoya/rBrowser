//! The grid area's frame (RB-010-12, contract 0.1-w1b): a panel around the grid whose inset
//! shows the environment's colour around the views. The grid always fits the window (flex
//! BoxLayouts), so zoom to fit is the cells' zoom (`presets`), and there is nothing to pan.
//! UI thread.

use std::cell::RefCell;

use cef::*;

use crate::presets::frame_argb;

/// The width of the frame in DIP.
const FRAME_WIDTH: i32 = 3;

#[derive(Default)]
struct FrameState {
    /// The panel around the grid, once the window built it.
    panel: Option<Panel>,
    /// The grid's own background: the frame's colour without an environment colour.
    neutral: u32,
    /// The colour asked for before the window existed.
    pending: Option<u32>,
    /// The environment's colour in effect, painted again after a theme change (RB-080-95).
    color: Option<u32>,
}

thread_local! {
    static FRAME: RefCell<FrameState> = RefCell::new(FrameState::default());
}

/// Frames the views in `color` (`#rrggbb`), or removes the frame (`None`, or a malformed
/// colour, which `presets` never sends).
pub fn set_color(color: Option<String>) {
    let argb = color.as_deref().and_then(frame_argb);
    FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        frame.color = argb;
        match &frame.panel {
            Some(panel) => panel.set_background_color(argb.unwrap_or(frame.neutral)),
            None => frame.pending = argb,
        }
    });
}

/// rBrowser's theme changed (RB-080-95) and reset the panel to the theme's ground: `ground`
/// (ARGB) is the neutral frame now, and the environment's colour is painted again.
pub fn theme_changed(ground: u32) {
    FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        frame.neutral = ground;
        if let Some(panel) = &frame.panel {
            panel.set_background_color(frame.color.unwrap_or(ground));
        }
    });
}

/// The view the window puts where the grid goes: a panel holding the grid with an inset of
/// [`FRAME_WIDTH`] on every side, painted in the environment's colour.
pub fn wrap_grid(grid: &Panel) -> View {
    let Some(panel) = super::views::sized_panel(0, 0, false) else {
        tracing::error!("the environment frame could not be created");
        return View::from(grid);
    };
    let layout = panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        inside_border_insets: Insets {
            top: FRAME_WIDTH,
            left: FRAME_WIDTH,
            bottom: FRAME_WIDTH,
            right: FRAME_WIDTH,
        },
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let mut child = View::from(grid);
    panel.add_child_view(Some(&mut child));
    if let Some(layout) = &layout {
        layout.set_flex_for_view(Some(&mut child), 1);
    }
    let neutral = grid.background_color();
    FRAME.with(|frame| {
        let mut frame = frame.borrow_mut();
        let pending = frame.pending.take();
        frame.color = pending;
        panel.set_background_color(pending.unwrap_or(neutral));
        frame.neutral = neutral;
        frame.panel = Some(panel.clone());
    });
    View::from(&panel)
}

/// Lets the frame's panel go (the main window was destroyed, RB-080-91).
pub fn release() {
    FRAME.with(|frame| *frame.borrow_mut() = FrameState::default());
}
