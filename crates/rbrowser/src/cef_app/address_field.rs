//! The address field of a decoupled view's label strip (RB-092-03, [`crate::cell_info::edit`]):
//! a native text field in the strip beside the view's name, hidden until a click on the
//! address piece, `view.edit_address` (`Leader S L`) or the page menu opens it with the view's
//! address selected. It takes the address piece's place, never a place over the view. Enter hands
//! the text to `view.edit_address`, which checks it as the address bar's input and loads it in
//! that view only: the field closes, or stays open while the status bar names the refusal.
//! Escape and a focus elsewhere close it and the view keeps its page. The keys' meaning is
//! [`crate::cell_info::address::field_key`]. UI thread only.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewId;
use serde_json::json;

use super::task::on_ui;
use super::views::{core, strip};
use crate::cell_info::address::{EDIT_ADDRESS, FieldKey, field_key};

thread_local! {
    /// Each strip's field (the grid built them).
    static FIELDS: RefCell<HashMap<ViewId, Textfield>> = RefCell::new(HashMap::new());
    /// The views whose field is open.
    static OPEN: RefCell<HashSet<ViewId>> = RefCell::new(HashSet::new());
}

wrap_textfield_delegate! {
    // The field of one strip: Enter loads, Escape and a focus elsewhere close.
    struct FieldDelegate {
        view: ViewId,
        height: i32,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            strip(0, self.height)
        }

        fn on_blur(&self, _view: Option<&mut View>) {
            close(self.view);
        }
    }

    impl TextfieldDelegate {
        fn on_key_event(
            &self,
            textfield: Option<&mut Textfield>,
            event: Option<&KeyEvent>,
        ) -> c_int {
            let (Some(textfield), Some(event)) = (textfield, event) else {
                return 0;
            };
            let down =
                event.type_ == KeyEventType::RAWKEYDOWN || event.type_ == KeyEventType::KEYDOWN;
            match field_key(event.windows_key_code, down) {
                Some(FieldKey::Submit) => {
                    submit(self.view, textfield);
                    1
                }
                Some(FieldKey::Cancel) => {
                    close(self.view);
                    1
                }
                None => 0,
            }
        }
    }
}

/// The hidden field of `view`'s strip, `height` DIP tall; kept for [`open`].
pub fn create(view: ViewId, height: i32) -> Option<Textfield> {
    let mut delegate = FieldDelegate::new(view, height);
    let field = textfield_create(Some(&mut delegate))?;
    View::from(&field).set_visible(0);
    FIELDS.with(|fields| {
        fields.borrow_mut().insert(view, field.clone());
    });
    Some(field)
}

/// Whether the field of `view` is open: its strip hides the address piece meanwhile.
pub fn is_open(view: ViewId) -> bool {
    OPEN.with(|open| open.borrow().contains(&view))
}

/// Opens the field of `view` with `url` selected, `name` its accessible name, and focuses it. A
/// view without a strip in the grid (popped out, the grid not built yet) does nothing.
pub fn open(view: ViewId, url: &str, name: &str) {
    let Some(field) = FIELDS.with(|fields| fields.borrow().get(&view).cloned()) else {
        return;
    };
    OPEN.with(|open| open.borrow_mut().insert(view));
    field.set_text(Some(&CefString::from(url)));
    field.set_accessible_name(Some(&CefString::from(name)));
    field.set_placeholder_text(Some(&CefString::from(
        rb_i18n::t("cells.address.placeholder").as_str(),
    )));
    field.select_all(0);
    let shown = View::from(&field);
    shown.set_visible(1);
    super::cell_info::redraw(view);
    shown.request_focus();
}

/// Closes the field of `view`; its address piece shows again.
pub fn close(view: ViewId) {
    if !OPEN.with(|open| open.borrow_mut().remove(&view)) {
        return;
    }
    if let Some(field) = FIELDS.with(|fields| fields.borrow().get(&view).cloned()) {
        View::from(&field).set_visible(0);
    }
    super::cell_info::redraw(view);
}

/// Enter: the text to `view.edit_address`, off the UI thread; the field closes once the view
/// loads it. A refusal leaves it open - the command put the reason into the status bar.
fn submit(view: ViewId, field: &Textfield) {
    let Some(core) = core() else {
        return;
    };
    let typed = CefString::from(&field.text()).to_string();
    let args = json!({ "view": view.0, "url": typed });
    core.run_command_async(EDIT_ADDRESS, args, move |result| match result {
        Ok(_) => on_ui(move || close(view)),
        Err(error) => tracing::debug!(%view, %error, "the label strip's address was refused"),
    });
}

/// Lets the fields go: the grid is built anew, its strips with them.
pub fn clear() {
    OPEN.with(|open| open.borrow_mut().clear());
    FIELDS.with(|fields| fields.borrow_mut().clear());
}
