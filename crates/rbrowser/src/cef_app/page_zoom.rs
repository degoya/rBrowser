//! The page zoom in the window (RB-090-20, [`crate::page_zoom`]): sets a view's level
//! (`CefBrowserHost::SetZoomLevel`), shows it as a badge in the view's label strip ("110%",
//! the reason as its tooltip, hidden at 100 %; a click sets it back), takes Ctrl+0, Ctrl+Plus
//! and Ctrl+Minus in a page ([`on_key`], from `keys.rs`), and reads every view's level a few
//! times a second: Chromium zooms on Ctrl+wheel without a callback to rBrowser, and it keeps a
//! zoom per site, so one view's zoom can move another's. A level that changed without rBrowser
//! goes to the model ([`crate::page_zoom::observed`]); a level that moved because rBrowser set a
//! neighbour's is reported as a follower, so the wheel's guard never fights Chromium. UI thread
//! only.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewId;
use serde_json::json;

use super::task::on_ui_after;
use super::views::{UI, core};
use crate::page_zoom::{self, is_zoomed, percent_of};

/// How often the views' levels are read.
const POLL_MS: u32 = 250;
/// Two readings closer than this are the same.
const SAME: f64 = 1e-6;

thread_local! {
    /// Each cell's zoom badge (the grid built it).
    static BADGES: RefCell<HashMap<ViewId, LabelButton>> = RefCell::new(HashMap::new());
    /// Each view's level as last read or set here.
    static SEEN: RefCell<HashMap<ViewId, f64>> = RefCell::new(HashMap::new());
    /// Whether the reading runs.
    static POLLING: Cell<bool> = const { Cell::new(false) };
}

wrap_button_delegate! {
    // The zoom badge of a cell: a click sets the page back to 100 %.
    struct BadgeDelegate {
        view: ViewId,
        height: i32,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            Size {
                width: 0,
                height: self.height,
            }
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut Button>) {
            run(self.view, page_zoom::RESET);
        }
    }
}

/// Runs the page zoom command `id` for `view`: a key in its page, its badge's click.
fn run(view: ViewId, id: &'static str) {
    let Some(core) = core() else {
        return;
    };
    core.run_command_async(id, json!({ "target": view.0 }), move |result| {
        if let Err(error) = result {
            tracing::warn!(%view, command = id, %error, "the page zoom failed");
        }
    });
}

/// A key-down in `view`'s page: Ctrl+0, Ctrl+Plus or Ctrl+Minus zoom it while
/// `views.pageZoom.keys` is on; `1` when the key was taken.
pub fn on_key(view: ViewId, event: &KeyEvent) -> c_int {
    let down = event.type_ == KeyEventType::RAWKEYDOWN || event.type_ == KeyEventType::KEYDOWN;
    if !down {
        return 0;
    }
    let Some(action) = page_zoom::key_action(event.modifiers, event.windows_key_code) else {
        return 0;
    };
    if !core().is_some_and(|core| page_zoom::keys_on(core)) {
        return 0;
    }
    run(view, page_zoom::command_of(action));
    1
}

/// `strip` - the label strip of `view` - with the zoom badge at its right end, shown while the
/// page is zoomed. Starts the reading of the levels.
pub fn with_badge(strip: View, view: ViewId, height: u32) -> View {
    ensure_polling();
    let mut delegate = BadgeDelegate::new(view, i32::try_from(height).unwrap_or(i32::MAX));
    let (Some(panel), Some(button)) = (
        panel_create(None),
        label_button_create(Some(&mut delegate), Some(&CefString::from(""))),
    ) else {
        return strip;
    };
    show(&button, SEEN.with(|seen| seen.borrow().get(&view).copied()));
    BADGES.with(|badges| {
        badges.borrow_mut().insert(view, button.clone());
    });
    let layout = panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let mut strip = strip;
    panel.add_child_view(Some(&mut strip));
    let mut button = View::from(&button);
    panel.add_child_view(Some(&mut button));
    if let Some(layout) = &layout {
        layout.set_flex_for_view(Some(&mut strip), 1);
        layout.set_flex_for_view(Some(&mut button), 0);
    }
    View::from(&panel)
}

/// The badge at `level`: its percent and the reason while zoomed, hidden at 100 %.
fn show(button: &LabelButton, level: Option<f64>) {
    let zoomed = level.is_some_and(is_zoomed);
    if let Some(level) = level.filter(|_| zoomed) {
        let percent = percent_of(level).to_string();
        let args = [("percent", percent.as_str())];
        button.set_text(Some(&CefString::from(
            rb_i18n::t_with("views.page_zoom.badge", &args).as_str(),
        )));
        button.set_tooltip_text(Some(&CefString::from(
            rb_i18n::t_with("views.page_zoom.tooltip", &args).as_str(),
        )));
    }
    View::from(button).set_visible(i32::from(zoomed));
}

/// Sets `view`'s page to `level` (the backend, from the model) and reports what moved with it.
pub fn set_level(view: ViewId, level: f64) {
    let Some(host) = browser_host(view) else {
        tracing::debug!(%view, "no browser for the page zoom (yet)");
        return;
    };
    host.set_zoom_level(level);
    // What moved to this very level moved with it (the same site); anything else changed on
    // its own since the last reading.
    let (followers, others): (Vec<_>, Vec<_>) = read_changes()
        .into_iter()
        .partition(|(_, read)| (read - level).abs() < 1e-3);
    page_zoom::followed(&followers);
    if let Some(core) = core() {
        for (view, read) in others {
            page_zoom::observed(core, view, read);
        }
    }
}

/// `view`'s page finished loading: its level as it is now goes to the model.
pub fn loaded(view: ViewId, browser: Option<&mut Browser>) {
    let Some(host) = browser.and_then(|browser| browser.host()) else {
        return;
    };
    let level = host.zoom_level();
    remember(view, level);
    if let Some(core) = core() {
        page_zoom::loaded(core, view, level);
    }
}

fn browser_host(view: ViewId) -> Option<BrowserHost> {
    UI.with(|ui| ui.borrow().views.get(&view).and_then(BrowserView::browser))
        .and_then(|browser| browser.host())
}

/// Takes `level` as `view`'s and shows it on its badge.
fn remember(view: ViewId, level: f64) {
    SEEN.with(|seen| seen.borrow_mut().insert(view, level));
    BADGES.with(|badges| {
        if let Some(button) = badges.borrow().get(&view) {
            show(button, Some(level));
        }
    });
}

/// Reads every view's level; the ones that differ from what was seen are remembered and
/// returned. Views that went are forgotten.
fn read_changes() -> Vec<(ViewId, f64)> {
    let read: Vec<(ViewId, f64)> = UI.with(|ui| {
        ui.borrow()
            .views
            .iter()
            .filter_map(|(id, view)| Some((*id, view.browser()?.host()?.zoom_level())))
            .collect()
    });
    SEEN.with(|seen| {
        seen.borrow_mut()
            .retain(|id, _| read.iter().any(|(view, _)| view == id));
    });
    let changed: Vec<(ViewId, f64)> = read
        .into_iter()
        .filter(|(view, level)| {
            SEEN.with(|seen| seen.borrow().get(view).copied())
                .is_none_or(|before| (before - level).abs() >= SAME)
        })
        .collect();
    for (view, level) in &changed {
        remember(*view, *level);
    }
    changed
}

/// Starts reading the levels, once per window.
fn ensure_polling() {
    if POLLING.with(|polling| polling.replace(true)) {
        return;
    }
    on_ui_after(POLL_MS, tick);
}

/// One reading: a change rBrowser did not make goes to the model; then the next one, while
/// the window lives.
fn tick() {
    if UI.with(|ui| ui.borrow().window.is_none()) {
        POLLING.with(|polling| polling.set(false));
        return;
    }
    let changed = read_changes();
    if let Some(core) = core() {
        for (view, level) in changed {
            page_zoom::observed(core, view, level);
        }
    }
    on_ui_after(POLL_MS, tick);
}

/// Lets the badges go: the grid is built anew, its strips with it.
pub fn clear() {
    BADGES.with(|badges| badges.borrow_mut().clear());
}

/// Lets every badge go and forgets the levels (the main window was destroyed, RB-080-91).
pub fn release() {
    clear();
    SEEN.with(|seen| seen.borrow_mut().clear());
}
