//! The pieces of a label strip beside the view's name (RB-090-27, [`crate::cell_info`]): the
//! address and the external engine's window, each a native label button with its details as the
//! tooltip, hidden while it has nothing to say. While the engine badge shows, the engine's state
//! stands in the badge beside the engine's name instead (RB-090-71, `engine_badge.rs`); the Web
//! Vitals are the vitals bar's under the view (`cell_footers.rs`). The core hands every view's
//! pieces after a change ([`set`]); a strip redraws only the pieces that changed, and a strip
//! built anew (the grid rebuilt) takes the last ones at once. A click on the engine's piece
//! restarts a stopped engine or brings its window to the front; a click on the address makes
//! the view active, as a click on its strip - or, for a view with its own navigation, opens the
//! address field that takes the address's place in the strip (RB-092-03, `address_field.rs`).
//! UI thread only.

use std::cell::RefCell;
use std::collections::HashMap;

use cef::*;
use rb_engine::ViewId;

use super::views::{core, strip};
use crate::cell_info::{CellInfo, Piece, PieceAction};

/// Which piece a button shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Address,
    Engine,
}

/// The two buttons of one strip, in the strip's order.
struct Buttons {
    address: LabelButton,
    engine: LabelButton,
}

thread_local! {
    /// Each cell's buttons (the grid built them).
    static BUTTONS: RefCell<HashMap<ViewId, Buttons>> = RefCell::new(HashMap::new());
    /// Each view's pieces as the core sent them last.
    static LAST: RefCell<HashMap<ViewId, CellInfo>> = RefCell::new(HashMap::new());
}

wrap_button_delegate! {
    // One piece of a strip: the engine's runs its action, the others activate the view.
    struct PieceDelegate {
        view: ViewId,
        kind: Kind,
        height: i32,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, self.height)
        }
    }

    impl ButtonDelegate {
        fn on_button_pressed(&self, _button: Option<&mut Button>) {
            pressed(self.view, self.kind);
        }
    }
}

/// A click on the `kind` piece of `view`'s strip: what the core's pieces say it does
/// ([`crate::cell_info::board::run`], shared with the canvas board).
fn pressed(view: ViewId, kind: Kind) {
    let action = LAST
        .with(|last| {
            last.borrow()
                .get(&view)
                .and_then(|info| piece_of(info, kind).map(|piece| piece.action))
        })
        .unwrap_or(PieceAction::Activate);
    if let Some(core) = core() {
        crate::cell_info::board::run(core, view, action);
    }
}

fn piece_of(info: &CellInfo, kind: Kind) -> Option<&Piece> {
    match kind {
        Kind::Address => info.address.as_ref(),
        Kind::Engine => info.engine.as_ref(),
    }
}

/// `strip` - the label strip of `view` - with its pieces after the name: the address, which
/// gives way first in a narrow cell, then the engine's window.
pub fn with_info(strip: View, view: ViewId, height: u32) -> View {
    let height = i32::try_from(height).unwrap_or(i32::MAX);
    let button = |kind: Kind| {
        let mut delegate = PieceDelegate::new(view, kind, height);
        label_button_create(Some(&mut delegate), Some(&CefString::from("")))
    };
    let (Some(panel), Some(address), Some(engine), Some(field)) = (
        panel_create(None),
        button(Kind::Address),
        button(Kind::Engine),
        super::address_field::create(view, height),
    ) else {
        return strip;
    };
    let buttons = Buttons { address, engine };
    LAST.with(|last| show_all(view, &buttons, last.borrow().get(&view)));
    let layout = panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let mut strip = strip;
    panel.add_child_view(Some(&mut strip));
    let mut children = [
        (View::from(&buttons.address), 1),
        (View::from(&field), 1),
        (View::from(&buttons.engine), 0),
    ];
    for (child, _) in &mut children {
        panel.add_child_view(Some(child));
    }
    if let Some(layout) = &layout {
        layout.set_flex_for_view(Some(&mut strip), 1);
        for (child, flex) in &mut children {
            layout.set_flex_for_view(Some(child), *flex);
        }
    }
    BUTTONS.with(|all| {
        all.borrow_mut().insert(view, buttons);
    });
    View::from(&panel)
}

/// Every piece of `info` on `view`'s buttons; all hidden without one. The engine's state in the
/// engine badge hides its own button; the open address field hides the address.
fn show_all(view: ViewId, buttons: &Buttons, info: Option<&CellInfo>) {
    show(
        &buttons.address,
        info.and_then(|info| info.address.as_ref())
            .filter(|_| !super::address_field::is_open(view)),
    );
    let engine = info
        .filter(|info| !info.engine_in_badge)
        .and_then(|info| info.engine.as_ref());
    show(&buttons.engine, engine);
}

/// The engine's state of `view` as the core sent it last, when it stands in the engine badge.
pub fn badge_state(view: ViewId) -> Option<Piece> {
    LAST.with(|last| {
        last.borrow()
            .get(&view)
            .filter(|info| info.engine_in_badge)
            .and_then(|info| info.engine.clone())
    })
}

/// `piece` on `button`, or the button hidden.
fn show(button: &LabelButton, piece: Option<&Piece>) {
    if let Some(piece) = piece {
        button.set_text(Some(&CefString::from(piece.text.as_str())));
        button.set_tooltip_text(Some(&CefString::from(piece.tooltip.as_str())));
        button.set_accessible_name(Some(&CefString::from(
            format!("{}. {}", piece.text, piece.tooltip).as_str(),
        )));
    }
    View::from(button).set_visible(i32::from(piece.is_some()));
}

/// The core's pieces of every view: the strips whose pieces changed redraw them; the views gone
/// are forgotten.
pub fn set(infos: Vec<CellInfo>) {
    let mut changed: Vec<CellInfo> = Vec::new();
    LAST.with(|last| {
        let mut last = last.borrow_mut();
        last.retain(|view, _| infos.iter().any(|info| info.view == *view));
        for info in infos {
            if last.get(&info.view) != Some(&info) {
                changed.push(info.clone());
                last.insert(info.view, info);
            }
        }
    });
    if changed.is_empty() {
        return;
    }
    BUTTONS.with(|all| {
        let all = all.borrow();
        for info in &changed {
            if let Some(buttons) = all.get(&info.view) {
                show_all(info.view, buttons, Some(info));
            }
        }
    });
    for info in &changed {
        let state = info.engine.as_ref().filter(|_| info.engine_in_badge);
        super::engine_badge::set_state(info.view, state);
    }
}

/// The pieces of `view`'s strip drawn again from the last ones: its address field opened or
/// closed.
pub fn redraw(view: ViewId) {
    let info = LAST.with(|last| last.borrow().get(&view).cloned());
    BUTTONS.with(|all| {
        if let Some(buttons) = all.borrow().get(&view) {
            show_all(view, buttons, info.as_ref());
        }
    });
}

/// Lets the buttons and the address fields go: the grid is built anew, its strips with it (the
/// pieces stay).
pub fn clear() {
    BUTTONS.with(|all| all.borrow_mut().clear());
    super::address_field::clear();
}

/// Lets the buttons and the pieces go (the main window was destroyed, RB-080-91).
pub fn release() {
    clear();
    LAST.with(|last| last.borrow_mut().clear());
}
