//! Native dialogs (RB-010-08, contract 0.1-w1b): the file and folder picker of the settings
//! window, CEF's own (`BrowserHost::run_file_dialog`, the Windows dialog), opened over the
//! pages window or, while it is closed, over the main window. UI thread.
//!
//! The window-free part - what is asked for, the one answer - is `crate::host::path_pick`
//! (RB-080-19): the filter list is always allocated (CEF reads it without a null check), the
//! returned list is read in place (a clone of a borrowed `CefStringList` is empty), and the
//! waiting page is answered exactly once, cancelled when CEF releases the callback unasked.

use std::sync::Arc;

use cef::sys::_cef_string_list_t;
use cef::*;

use super::settings_window;
use super::views::UI;
use crate::host::path_pick::{DialogMode, DialogRequest, PickOnce};
use crate::host::ui::{PathPick, PathPicked};

wrap_run_file_dialog_callback! {
    // Hands the first chosen path (none when cancelled) to the waiting page, once.
    struct Picked {
        once: Arc<PickOnce>,
    }

    impl RunFileDialogCallback {
        fn on_file_dialog_dismissed(&self, file_paths: Option<&mut CefStringList>) {
            self.once.finish(file_paths.map(borrowed).into_iter().flatten());
        }
    }
}

/// The same list, borrowed for reading: `IntoIterator` consumes a `CefStringList`, and a
/// clone of a borrowed one has no list behind it; one built from the raw pointer reads the
/// original and frees nothing on drop.
fn borrowed(list: &mut CefStringList) -> CefStringList {
    let raw: *mut _cef_string_list_t = list.into();
    CefStringList::from(raw)
}

/// Shows the dialog `pick` and calls `done` with the choice, `None` when cancelled or when no
/// window is there to show it over.
pub fn pick_path(pick: PathPick, done: PathPicked) {
    let once = Arc::new(PickOnce::new(done));
    let host = settings_window::browser()
        .or_else(|| UI.with(|ui| ui.borrow().shell.as_ref().and_then(BrowserView::browser)))
        .and_then(|browser| browser.host());
    let Some(host) = host else {
        tracing::warn!("no window to show the path dialog over");
        once.finish(Vec::new());
        return;
    };
    let request = DialogRequest::from(&pick);
    let mode = match request.mode {
        DialogMode::OpenFile => FileDialogMode::OPEN,
        DialogMode::OpenFolder => FileDialogMode::OPEN_FOLDER,
        DialogMode::Save => FileDialogMode::SAVE,
    };
    let mut filters = CefStringList::new();
    for filter in &request.filters {
        filters.append(filter);
    }
    let mut callback = Picked::new(once);
    host.run_file_dialog(
        mode,
        Some(&CefString::from(request.title.as_str())),
        request
            .default_path
            .as_deref()
            .map(CefString::from)
            .as_ref(),
        Some(&mut filters),
        Some(&mut callback),
    );
}
