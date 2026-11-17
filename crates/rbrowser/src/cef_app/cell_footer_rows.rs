//! The lines of a footer strip below the breakpoint bar, on the theme's ground and text rather
//! than the breakpoint's colour (`cef_app/cell_footers.rs` builds and writes the strip):
//!
//! - **The vitals bar** (RB-090-71, `ui.viewFooter.vitals`): one button naming the view's Core
//!   Web Vitals with the shapes of their rating, every vital in words as the tooltip; a click
//!   marks the element of the worst rated vital - what the host's footer says it does
//!   (`Host::footer_vitals_action`, [`crate::cell_info::board::run`]).
//! - **The Lighthouse bar** (RB-090-73, `ui.viewFooter.lighthouse`): a row of one button per
//!   category score, side by side in the middle of the strip, each in Lighthouse's colour for
//!   its rating on the theme's ground with the shape of the rating before the score
//!   (`crate::view_footer::lighthouse`: a native button draws text only, so a gauge is its shape,
//!   its score and its category's name - as fully as the strip's width holds). The tooltip names
//!   the run's time and every score in words; a click opens the run's report
//!   (`crate::view_footer::lighthouse_runs::open_report`).
//!
//! CEF resets a view's colours to its theme's at every theme change; each delegate paints again
//! in `on_theme_changed`. UI thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cef::*;
use rb_engine::ViewId;

use super::super::views::{core, strip};
use crate::app_theme::opaque;
use crate::view_footer::CellFooter;
use crate::view_footer::lighthouse::{Category, gauge_color, gauge_text, spoken};

/// A gauge's score as its delegate keeps it: none.
const NO_SCORE: u32 = u32::MAX;

wrap_button_delegate! {
    // The vitals bar of a footer (RB-090-71): a click marks the element of the worst vital.
    struct VitalsDelegate {
        view: ViewId,
        height: Arc<AtomicU32>,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, i32::try_from(self.height.load(Ordering::Relaxed)).unwrap_or(i32::MAX))
        }

        // The theme's ground and text, after every theme change: CEF reset them.
        fn on_theme_changed(&self, view: Option<&mut View>) {
            let button = view
                .and_then(|view| view.as_button())
                .and_then(|button| button.as_label_button());
            if let Some(button) = button {
                paint_vitals(&button);
            }
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut Button>) {
            if let Some(core) = core() {
                let action = core.host().footer_vitals_action(self.view);
                crate::cell_info::board::run(core, self.view, action);
            }
        }
    }
}

wrap_panel_delegate! {
    // The Lighthouse bar of a footer (RB-090-73): its gauges side by side, one line high.
    struct LighthouseRow {
        height: Arc<AtomicU32>,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, i32::try_from(self.height.load(Ordering::Relaxed)).unwrap_or(i32::MAX))
        }

        // The theme's ground behind the gauges, after every theme change.
        fn on_theme_changed(&self, view: Option<&mut View>) {
            if let Some(panel) = view.and_then(|view| view.as_panel()) {
                panel.set_background_color(opaque(super::super::theme::colors().ground));
            }
        }
    }

    impl PanelDelegate {}
}

wrap_button_delegate! {
    // One gauge of the Lighthouse bar: as wide as its words; a click opens the run's report.
    struct GaugeDelegate {
        view: ViewId,
        score: Arc<AtomicU32>,
    }

    impl ViewDelegate {
        // Lighthouse's colour on the theme's ground, after every theme change.
        fn on_theme_changed(&self, view: Option<&mut View>) {
            let button = view
                .and_then(|view| view.as_button())
                .and_then(|button| button.as_label_button());
            if let Some(button) = button {
                paint_gauge(&button, self.score.load(Ordering::Relaxed));
            }
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut Button>) {
            if let Some(core) = core() {
                crate::view_footer::lighthouse_runs::open_report(core, self.view);
            }
        }
    }
}

/// The vitals bar in the theme's colours: the label strips' ground and text.
fn paint_vitals(button: &LabelButton) {
    let colors = super::super::theme::colors();
    button.set_background_color(opaque(colors.ground));
    button.set_enabled_text_colors(opaque(colors.text));
}

/// A gauge on the theme's ground in Lighthouse's colour of `score` ([`NO_SCORE`]: the theme's
/// text).
fn paint_gauge(button: &LabelButton, score: u32) {
    let colors = super::super::theme::colors();
    let score = u8::try_from(score).ok();
    let text = gauge_color(score, colors.ground).unwrap_or(colors.text);
    button.set_background_color(opaque(colors.ground));
    button.set_enabled_text_colors(opaque(text));
}

/// The vitals bar's button of `view`'s strip, `line` DIP high.
pub(super) fn vitals_button(view: ViewId, line: Arc<AtomicU32>) -> Option<LabelButton> {
    let mut delegate = VitalsDelegate::new(view, line);
    let vitals = label_button_create(Some(&mut delegate), Some(&CefString::from("")))?;
    vitals.set_horizontal_alignment(HorizontalAlignment::CENTER);
    Some(vitals)
}

/// The vitals bar of `footer` on `button`, or the button hidden: no bar, or no room for it.
pub(super) fn write_vitals(button: &LabelButton, footer: &CellFooter) {
    let shown = footer.vitals.as_ref().filter(|_| footer.vitals_shown);
    if let Some(piece) = shown {
        button.set_text(Some(&CefString::from(piece.text.as_str())));
        button.set_tooltip_text(Some(&CefString::from(piece.tooltip.as_str())));
        button.set_accessible_name(Some(&CefString::from(
            format!("{}. {}", piece.text, piece.tooltip).as_str(),
        )));
        paint_vitals(button);
    }
    set_shown(&View::from(button), shown.is_some());
}

/// The Lighthouse bar of a strip: its row and a button per category with the score it shows.
#[derive(Clone)]
pub(super) struct LighthouseLine {
    pub(super) panel: Panel,
    gauges: Vec<(LabelButton, Arc<AtomicU32>)>,
}

/// The Lighthouse bar of `view`'s strip, `line` DIP high, its gauges in `font`; hidden until a
/// footer has one.
pub(super) fn lighthouse_line(
    view: ViewId,
    line: Arc<AtomicU32>,
    font: &CefString,
) -> Option<LighthouseLine> {
    let mut delegate = LighthouseRow::new(line);
    let panel = panel_create(Some(&mut delegate))?;
    panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        main_axis_alignment: AxisAlignment::CENTER,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let mut gauges = Vec::with_capacity(Category::ALL.len());
    for _ in Category::ALL {
        let score = Arc::new(AtomicU32::new(NO_SCORE));
        let mut delegate = GaugeDelegate::new(view, score.clone());
        let button = label_button_create(Some(&mut delegate), Some(&CefString::from("")))?;
        button.set_font_list(Some(font));
        button.set_horizontal_alignment(HorizontalAlignment::CENTER);
        panel.add_child_view(Some(&mut View::from(&button)));
        gauges.push((button, score));
    }
    View::from(&panel).set_visible(0);
    Some(LighthouseLine { panel, gauges })
}

/// The text size changed: the gauges take `font` and are laid out again.
pub(super) fn lighthouse_font(line: &LighthouseLine, font: &CefString) {
    for (button, _) in &line.gauges {
        button.set_font_list(Some(font));
        View::from(button).invalidate_layout();
    }
    View::from(&line.panel).invalidate_layout();
}

/// The Lighthouse bar of `footer` on `line`: a gauge per category it scored, in the form its
/// width holds; the row hidden without a bar or without room for it.
pub(super) fn write_lighthouse(line: &LighthouseLine, footer: &CellFooter) {
    let shown = footer
        .lighthouse
        .as_ref()
        .filter(|_| footer.lighthouse_shown);
    if let Some(bar) = shown {
        for (index, (button, score)) in line.gauges.iter().enumerate() {
            let gauge = bar.gauges.get(index);
            if let Some(gauge) = gauge {
                let text = gauge_text(gauge, footer.lighthouse_form);
                button.set_text(Some(&CefString::from(text.as_str())));
                button.set_tooltip_text(Some(&CefString::from(bar.tooltip.as_str())));
                button.set_accessible_name(Some(&CefString::from(spoken(gauge).as_str())));
                score.store(gauge.score.map_or(NO_SCORE, u32::from), Ordering::Relaxed);
                paint_gauge(button, score.load(Ordering::Relaxed));
            }
            set_shown(&View::from(button), gauge.is_some());
        }
        line.panel
            .set_background_color(opaque(super::super::theme::colors().ground));
        View::from(&line.panel).invalidate_layout();
    }
    set_shown(&View::from(&line.panel), shown.is_some());
}

/// Shows or hides `view`, touching it only when that changes.
fn set_shown(view: &View, shown: bool) {
    let visible = i32::from(shown);
    if view.is_visible() != visible {
        view.set_visible(visible);
    }
}
