//! The footer under each view (RB-080-44, `crate::view_footer`): a native strip in the
//! breakpoint's colour below the view - in its grid cell, in the overlay of a layout mode (the
//! canvas included: the strip takes no mouse work, so it stays native there) and at the bottom
//! of a view's own window. What it says and its colours come from the host
//! (`crate::host::footer`), which writes a strip again whenever its view's size, breakpoint or
//! the page's analysis changes. The grid builds a strip only for a view that has a footer; a
//! view's own window always holds one and hides it, so the switch reaches it without a new
//! window - the window then grows or shrinks by the strip, so the page keeps its size.
//!
//! The colours (RB-080-47): CEF resets a view's background to its theme's whenever the view's
//! theme changes, and coming into a window - the strip itself or a panel above it - is such a
//! change. A strip is built before it is in its cell, so the colour set then is gone when the
//! strip shows: the strip had the window's dark ground, in an overlay (the canvas) none at all,
//! under a text chosen for the colour that was never drawn. The strip's delegate therefore
//! paints in `on_theme_changed`, from the colours kept beside the strip; what is painted when
//! is `crate::view_footer::paint`, tested there with a strip that loses its ground.
//!
//! The lines (RB-080-53): a CEF label button draws one line and cuts what does not fit its
//! width with an ellipsis - in a narrow view the queries were cut off. A strip is therefore a
//! panel of [`MAX_LINES`] buttons stacked, one per line the host decided
//! (`crate::view_footer::wrap`); a line not used is hidden and the panel is that much lower.
//! The text stands in the middle of each button, or at its left (`ui.viewFooter.align`). The
//! first strip laid out in a run logs what CEF measures for its first line beside the host's
//! estimate and the width it has - once, then never again. UI thread.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use cef::*;
use rb_engine::ViewId;

use super::views::strip;
use crate::view_footer::paint::{self, Paintable};
use crate::view_footer::wrap::{MAX_LINES, text_width};
use crate::view_footer::{CellFooter, FOOTER_HEIGHT, StripColors, window_height};

/// The strip's font: monospace, so the fields of the views line up. The host's estimate of a
/// line's width (`view_footer::wrap`) holds for this font.
const FONT: &str = "Consolas, Courier New, monospace, 11px";

/// A strip: the panel of its lines, the colours its delegates paint it with and its height.
#[derive(Clone)]
struct Strip {
    panel: Panel,
    lines: Vec<LabelButton>,
    colors: StripColors,
    height: Arc<AtomicU32>,
}

/// The first strip of the run measured its line (RB-080-53).
static MEASURED: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// The strip of each view in the grid; built anew with every grid.
    static GRID: RefCell<HashMap<ViewId, Strip>> = RefCell::new(HashMap::new());
    /// The strip in the window of each popped-out view.
    static WINDOWS: RefCell<HashMap<ViewId, Strip>> = RefCell::new(HashMap::new());
}

/// The footers drawn in the grid or on the canvas, as rectangles in screen DIP: where Ctrl +
/// wheel zooms the canvas though no page hears it (RB-080-50, `board_pan.rs`).
#[cfg_attr(
    target_os = "macos",
    allow(
        dead_code,
        reason = "used by the Windows-only canvas pan; its macOS twin is a no-op"
    )
)]
pub fn drawn_rects() -> Vec<Rect> {
    GRID.with(|strips| {
        strips
            .borrow()
            .values()
            .map(|strip| View::from(&strip.panel))
            .filter(|strip| strip.window().is_some() && strip.is_drawn() != 0)
            .map(|strip| strip.bounds_in_screen())
            .collect()
    })
}

wrap_panel_delegate! {
    // The strip: its lines stacked, as tall as the lines shown.
    struct FooterPanel {
        height: Arc<AtomicU32>,
        colors: StripColors,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, i32::try_from(self.height.load(Ordering::Relaxed)).unwrap_or(i32::MAX))
        }

        // Behind the lines: the footer's ground too, after every theme change.
        fn on_theme_changed(&self, view: Option<&mut View>) {
            if let Some(panel) = view.and_then(|view| view.as_panel()) {
                paint::theme_changed(&panel, &self.colors);
            }
        }
    }

    impl PanelDelegate {}
}

wrap_button_delegate! {
    // One line of a footer: text, nothing to press.
    struct FooterDelegate {
        height: i32,
        colors: StripColors,
        // The first line of a strip: the first one of the run logs its measure.
        first: bool,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, self.height)
        }

        // The theme's colours were just applied over the strip's - it came into a window, or
        // the window's theme changed: the footer's colours again.
        fn on_theme_changed(&self, view: Option<&mut View>) {
            let button = view
                .and_then(|view| view.as_button())
                .and_then(|button| button.as_label_button());
            if let Some(button) = button {
                paint::theme_changed(&button, &self.colors);
                tracing::debug!(
                    background = %format_args!("{:08x}", button.background_color()),
                    "a view footer painted after a theme change"
                );
            }
        }

        fn on_layout_changed(&self, view: Option<&mut View>, new_bounds: Option<&Rect>) {
            let (true, Some(view), Some(bounds)) = (self.first, view, new_bounds) else {
                return;
            };
            if bounds.width > 0 && !MEASURED.swap(true, Ordering::Relaxed) {
                log_measure(view, bounds.width);
            }
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut Button>) {}
    }
}

/// What CEF measures for the first line of a strip beside the host's estimate and the width
/// the strip has (RB-080-53), once per run: the evidence for the estimate's font. The
/// strip's own button reports its delegate's size, so a button of the same font that is never
/// shown measures the line.
fn log_measure(view: &View, available: i32) {
    let Some(button) = view.as_button().and_then(|button| button.as_label_button()) else {
        return;
    };
    let line = CefString::from(&button.text()).to_string();
    let measured = label_button_create(None, Some(&CefString::from(line.as_str()))).map(|probe| {
        probe.set_font_list(Some(&CefString::from(FONT)));
        View::from(&probe).preferred_size().width
    });
    tracing::info!(
        chars = line.chars().count(),
        measured = ?measured,
        estimated = text_width(&line),
        available,
        "a view footer line was laid out"
    );
}

/// A strip of `line` DIP per line saying `footer`.
fn build(footer: &CellFooter, line: u32) -> Option<Strip> {
    let colors = StripColors::of(footer);
    let height = Arc::new(AtomicU32::new(footer.height()));
    let mut delegate = FooterPanel::new(height.clone(), colors.clone());
    let panel = panel_create(Some(&mut delegate))?;
    panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 0,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let line = i32::try_from(line).unwrap_or(i32::MAX);
    let mut lines = Vec::with_capacity(MAX_LINES);
    for index in 0..MAX_LINES {
        let mut delegate = FooterDelegate::new(line, colors.clone(), index == 0);
        let button = label_button_create(Some(&mut delegate), Some(&CefString::from("")))?;
        button.set_font_list(Some(&CefString::from(FONT)));
        // Text only: the Tab key passes it by.
        button.set_focusable(0);
        panel.add_child_view(Some(&mut View::from(&button)));
        lines.push(button);
    }
    let strip = Strip {
        panel,
        lines,
        colors,
        height,
    };
    write(&strip, footer);
    Some(strip)
}

/// Writes `footer` into `strip`: its lines, tooltip, alignment and colours; a line not used is
/// hidden, and a strip whose height changed is laid out again with the view beside it.
fn write(strip: &Strip, footer: &CellFooter) {
    let align = if footer.centered {
        HorizontalAlignment::CENTER
    } else {
        HorizontalAlignment::LEFT
    };
    for (index, button) in strip.lines.iter().enumerate() {
        let text = match footer.lines.get(index) {
            Some(line) => Some(line.as_str()),
            None if index == 0 => Some(footer.text.as_str()),
            None => None,
        };
        button.set_text(Some(&CefString::from(text.unwrap_or(""))));
        button.set_tooltip_text(Some(&CefString::from(footer.tooltip.as_str())));
        button.set_horizontal_alignment(align);
        let shown = i32::from(text.is_some());
        let line = View::from(button);
        if line.is_visible() != shown {
            line.set_visible(shown);
        }
        paint::written(button, &strip.colors, footer);
    }
    paint::written(&strip.panel, &strip.colors, footer);
    if strip.height.swap(footer.height(), Ordering::Relaxed) != footer.height() {
        let panel = View::from(&strip.panel);
        panel.invalidate_layout();
        if let Some(parent) = panel.parent_view() {
            parent.invalidate_layout();
        }
    }
}

impl Paintable for LabelButton {
    fn set_ground(&self, argb: u32) {
        self.set_background_color(argb);
    }

    // Every state but disabled: a strip is never pressed, but the pointer hovers over it.
    fn set_text_color(&self, argb: u32) {
        self.set_enabled_text_colors(argb);
    }
}

impl Paintable for Panel {
    fn set_ground(&self, argb: u32) {
        self.set_background_color(argb);
    }

    // The panel draws no text: its lines do.
    fn set_text_color(&self, _argb: u32) {}
}

/// Forgets the grid's strips: the grid is built anew.
pub fn clear() {
    GRID.with(|strips| strips.borrow_mut().clear());
}

/// The strip under `view` in its cell, `line` DIP per line of `footer`.
pub fn grid_strip(view: ViewId, footer: &CellFooter, line: u32) -> Option<View> {
    let strip = build(footer, line)?;
    let view_of = View::from(&strip.panel);
    GRID.with(|strips| strips.borrow_mut().insert(view, strip));
    Some(view_of)
}

/// The strip at the bottom of the window of `view`: hidden without `footer`, so the view fills
/// the window until `ui.viewFooter` shows one.
pub fn window_strip(view: ViewId, footer: Option<&CellFooter>) -> Option<View> {
    let strip = build(&footer.cloned().unwrap_or_default(), FOOTER_HEIGHT)?;
    let view_of = View::from(&strip.panel);
    view_of.set_visible(i32::from(footer.is_some()));
    WINDOWS.with(|strips| strips.borrow_mut().insert(view, strip));
    Some(view_of)
}

/// The DIP the footer takes of the window of `view` now: 0 while its strip is hidden.
pub fn window_footer(view: ViewId) -> u32 {
    WINDOWS.with(|strips| {
        strips
            .borrow()
            .get(&view)
            .filter(|strip| View::from(&strip.panel).is_visible() != 0)
            .map_or(0, |strip| strip.height.load(Ordering::Relaxed))
    })
}

/// The window of `view` is gone: its strip with it.
pub fn forget_window(view: ViewId) {
    WINDOWS.with(|strips| strips.borrow_mut().remove(&view));
}

/// Writes `footer` into the strip of `view` - in its cell and in its window - or hides it. A
/// window whose strip appears, goes or changes its lines grows or shrinks by the difference:
/// the page keeps its size.
pub fn set(view: ViewId, footer: Option<CellFooter>) {
    let grid = GRID.with(|strips| strips.borrow().get(&view).cloned());
    let window = WINDOWS.with(|strips| strips.borrow().get(&view).cloned());
    if let Some(strip) = &grid {
        show(strip, footer.as_ref());
    }
    if let Some(strip) = &window {
        let before = window_footer(view);
        show(strip, footer.as_ref());
        let after = window_footer(view);
        if before != after
            && let Some(window) = View::from(&strip.panel).window()
        {
            resize(&window, before, after);
        }
    }
}

/// Writes `footer` into `strip` and shows it, or hides it.
fn show(strip: &Strip, footer: Option<&CellFooter>) {
    if let Some(footer) = footer {
        write(strip, footer);
    }
    let strip = View::from(&strip.panel);
    let shown = footer.is_some();
    if (strip.is_visible() != 0) == shown {
        return;
    }
    strip.set_visible(i32::from(shown));
    // The view beside it takes or gives the strip's room.
    if let Some(parent) = strip.parent_view() {
        parent.invalidate_layout();
    }
}

/// Makes `window` taller or lower by its footer's change from `before` to `after` DIP. A
/// maximized or full-screen window keeps its bounds: there the page takes or gives the room.
fn resize(window: &Window, before: u32, after: u32) {
    if window.is_maximized() != 0 || window.is_fullscreen() != 0 || window.is_minimized() != 0 {
        return;
    }
    let bounds = window.bounds_in_screen();
    window.set_bounds(Some(&Rect {
        height: window_height(bounds.height, before, after),
        ..bounds
    }));
}

/// Lets every strip go, the grid's and the views' own windows' (the main window was destroyed,
/// RB-080-91): what a strip holds of CEF is released while CEF still runs.
pub fn release() {
    GRID.with(|strips| strips.borrow_mut().clear());
    WINDOWS.with(|strips| strips.borrow_mut().clear());
}
