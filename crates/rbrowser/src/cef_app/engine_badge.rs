//! The engine badge at the right end of each cell's label strip (RB-080-33): a native label
//! button naming the view's engine, dimmed with the reason as its tooltip while the engine
//! cannot run, and the native menu a click on it opens - Chromium, Firefox, WebKit, the view's
//! engine checked, one that cannot run disabled. What the badge says and what the menu holds come
//! from [`crate::breakpoint_keys::engine_pick`]; a choice runs `view.set_engine`. The canvas
//! board opens the same menu under the badge it draws (`canvas_board.rs`); the chord opens it
//! under the badge of the active view. `ui.engineBadge` shows or hides every badge. The state of
//! an external engine's window stands in the badge beside the engine's name (RB-090-71,
//! `Firefox`, a middle dot, `window closed`; [`set_state`]), its details before the badge's in
//! the tooltip, and the menu's first row does what a click on that state did before - open the
//! window again, restart the engine, show the window. UI thread only.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use cef::*;
use rb_engine::ViewId;

use super::views::core;
use crate::breakpoint_keys::{self, EngineItem};
use crate::cell_info::{Piece, PieceAction};

/// The first command id of the menu's rows (after the size quick pick's, `cell_labels.rs`).
const FIRST_ID: i32 = 26_600;
/// The menu row of the engine window's state (RB-090-71), past the engines' rows.
const STATE_ID: i32 = 26_690;
thread_local! {
    /// Each cell's badge (the grid built it).
    static BADGES: RefCell<HashMap<ViewId, LabelButton>> = RefCell::new(HashMap::new());
    /// `ui.engineBadge`.
    static SHOWN: Cell<bool> = const { Cell::new(true) };
    /// The badges whose engine cannot run: dimmed in the theme's muted text (RB-080-95).
    static DIMMED: RefCell<HashSet<ViewId>> = RefCell::new(HashSet::new());
    /// Each badge's own text and tooltip, under an engine window's state (RB-090-71).
    static OWN: RefCell<HashMap<ViewId, (String, String)>> = RefCell::new(HashMap::new());
}

wrap_button_delegate! {
    // The engine badge of a cell: a click opens the engine menu under it.
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
        fn on_button_pressed(&self, button: Option<&mut Button>) {
            let Some(button) = button else {
                return;
            };
            let bounds = button.bounds_in_screen();
            if let Some(window) = button.window() {
                show_pick_at(self.view, &window, under_right_end(bounds));
            }
        }
    }
}

wrap_menu_model_delegate! {
    // The engine menu of one view: a row's command id is `FIRST_ID` + its index, the engine
    // window's state row `STATE_ID`.
    struct EngineMenu {
        view: ViewId,
        items: Arc<Vec<EngineItem>>,
        state: Option<PieceAction>,
    }

    impl MenuModelDelegate {
        fn execute_command(
            &self,
            _menu_model: Option<&mut MenuModel>,
            command_id: std::os::raw::c_int,
            _event_flags: EventFlags,
        ) {
            let Some(core) = core() else {
                return;
            };
            if command_id == STATE_ID {
                if let Some(action) = self.state {
                    crate::cell_info::board::run(core, self.view, action);
                }
                return;
            }
            let item = command_id
                .checked_sub(FIRST_ID)
                .and_then(|index| usize::try_from(index).ok())
                .and_then(|index| self.items.get(index));
            if let Some(item) = item {
                breakpoint_keys::pick_engine(core, self.view, item);
            }
        }
    }
}

/// `strip` - the label strip of `view`, the session badge included - with the engine badge at
/// its right end; `strip` as it is when the view does not exist. A hidden badge
/// (`ui.engineBadge` off) is built too and shown when the setting changes.
pub fn with_badge(strip: View, view: ViewId, height: u32) -> View {
    let Some(badge) = core().and_then(|core| breakpoint_keys::badge_of(core, view)) else {
        return strip;
    };
    let mut delegate = BadgeDelegate::new(view, i32::try_from(height).unwrap_or(i32::MAX));
    let (Some(panel), Some(button)) = (
        panel_create(None),
        label_button_create(
            Some(&mut delegate),
            Some(&CefString::from(badge.text.as_str())),
        ),
    ) else {
        return strip;
    };
    button.set_tooltip_text(Some(&CefString::from(badge.tooltip.as_str())));
    let dimmed = !badge.available();
    if dimmed {
        button.set_enabled_text_colors(super::theme::muted_text());
    }
    DIMMED.with(|set| {
        let mut set = set.borrow_mut();
        if dimmed {
            set.insert(view);
        } else {
            set.remove(&view);
        }
    });
    View::from(&button).set_visible(i32::from(SHOWN.with(Cell::get)));
    BADGES.with(|badges| {
        badges.borrow_mut().insert(view, button.clone());
    });
    OWN.with(|own| {
        own.borrow_mut()
            .insert(view, (badge.text.clone(), badge.tooltip.clone()));
    });
    set_state(view, super::cell_info::badge_state(view).as_ref());
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

/// Opens the engine menu of `view` under its badge (the chord, the palette), or under the
/// right end of its label strip when the badge is hidden (`ui.engineBadge` off) or was not
/// built; nothing for a view in a window of its own.
pub fn show_pick(view: ViewId) {
    let badge = BADGES
        .with(|badges| badges.borrow().get(&view).cloned())
        .filter(|_| SHOWN.with(Cell::get))
        .map(|button| View::from(&button))
        .or_else(|| super::cell_labels::strip_of(view).map(|strip| View::from(&strip)));
    let Some(anchor) = badge else {
        return;
    };
    let bounds = anchor.bounds_in_screen();
    if let Some(window) = anchor.window() {
        show_pick_at(view, &window, under_right_end(bounds));
    }
}

/// Lets every badge go: the grid is built anew, its strips with it.
pub fn clear() {
    BADGES.with(|badges| badges.borrow_mut().clear());
    DIMMED.with(|dimmed| dimmed.borrow_mut().clear());
    OWN.with(|own| own.borrow_mut().clear());
}

/// The badge of `view` names the engine window's `state` beside the engine (RB-090-71) - one
/// badge, not two - with the words of [`crate::cell_info::badge_words`]; without a state the
/// badge's own words again.
pub fn set_state(view: ViewId, state: Option<&Piece>) {
    let Some((text, tooltip)) = OWN.with(|own| own.borrow().get(&view).cloned()) else {
        return;
    };
    // The dimmed look stays: the text colour is the badge's, only its words change.
    let dimmed = DIMMED.with(|dimmed| dimmed.borrow().contains(&view));
    let (text, tooltip) =
        crate::cell_info::badge_words((text.as_str(), tooltip.as_str()), dimmed, state);
    BADGES.with(|badges| {
        if let Some(button) = badges.borrow().get(&view) {
            button.set_text(Some(&CefString::from(text.as_str())));
            button.set_tooltip_text(Some(&CefString::from(tooltip.as_str())));
            button.set_accessible_name(Some(&CefString::from(
                format!("{text}. {tooltip}").as_str(),
            )));
            View::from(button).invalidate_layout();
        }
    });
}

/// rBrowser's theme changed (RB-080-95): the dimmed badges take its muted text.
pub fn theme_changed() {
    let muted = super::theme::muted_text();
    BADGES.with(|badges| {
        let badges = badges.borrow();
        DIMMED.with(|dimmed| {
            for view in dimmed.borrow().iter() {
                if let Some(button) = badges.get(view) {
                    button.set_enabled_text_colors(muted);
                }
            }
        });
    });
}

/// Shows the engine menu of `view` in `window` with its top-right corner at `point` (screen
/// DIP): under a badge, or under the right end of the strip the canvas board draws.
pub fn show_pick_at(view: ViewId, window: &Window, point: Point) {
    let Some(items) = core().and_then(|core| breakpoint_keys::engine_pick(core, view)) else {
        return;
    };
    let items = Arc::new(items);
    // What a click on the engine window's state did, first (RB-090-71).
    let state = core().and_then(|core| {
        let externals = core.host().shell_state().externals;
        let settings = core.user_settings().ui.cell_label;
        crate::cell_info::engine_menu_row(&externals, view, &settings)
    });
    let mut delegate = EngineMenu::new(view, items.clone(), state.map(|(action, _)| action));
    let Some(mut model) = menu_model_create(Some(&mut delegate)) else {
        return;
    };
    if let Some((_, key)) = state {
        model.add_item(STATE_ID, Some(&CefString::from(rb_i18n::t(key).as_str())));
        model.add_separator();
    }
    for (index, item) in items.iter().enumerate() {
        let Some(id) = i32::try_from(index)
            .ok()
            .and_then(|index| FIRST_ID.checked_add(index))
        else {
            break;
        };
        model.add_check_item(id, Some(&CefString::from(item.label.as_str())));
        model.set_checked(id, i32::from(item.checked));
        model.set_enabled(id, i32::from(item.available()));
    }
    super::theme::paint_menu(&model);
    window.show_menu(Some(&mut model), Some(&point), MenuAnchorPosition::TOPRIGHT);
}

/// `ui.engineBadge`: shows or hides every badge built so far; the ones built next follow too.
pub fn set_shown(shown: bool) {
    SHOWN.with(|cell| cell.set(shown));
    BADGES.with(|badges| {
        for button in badges.borrow().values() {
            View::from(button).set_visible(i32::from(shown));
        }
    });
}

/// The point a menu anchored top-right hangs from: the bottom-right corner of `bounds`.
fn under_right_end(bounds: Rect) -> Point {
    Point {
        x: bounds.x.saturating_add(bounds.width),
        y: bounds.y.saturating_add(bounds.height),
    }
}
