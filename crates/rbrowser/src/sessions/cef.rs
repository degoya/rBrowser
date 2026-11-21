//! The CEF side of sessions (Windows, UI thread): one request context per named session, kept
//! for the instance's life (CEF allows one context per cache folder), the deletion of a
//! session's cookies, and the session badge of a cell's label strip.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cef::*;

use super::{SessionBadge, argb, cache_path, profile};
use crate::cdp::lock;
use crate::host::ui::CookiesCleared;

thread_local! {
    /// The request context of each named session created so far.
    static CONTEXTS: RefCell<HashMap<String, RequestContext>> = RefCell::new(HashMap::new());
}

/// The request context a new browser of `session` is created in: `None` (the global context)
/// for `default`; the session's own, persistent in `<profile>\sessions\<id>`, for a named one.
/// A session whose folder cannot be derived gets a context in memory, never the shared one, so
/// a broken id cannot leak another session's login.
pub fn request_context(session: &str) -> Option<RequestContext> {
    if session == rb_config::DEFAULT_SESSION {
        return None;
    }
    if let Some(context) = CONTEXTS.with(|contexts| contexts.borrow().get(session).cloned()) {
        return Some(context);
    }
    let path = match profile().map(|profile| cache_path(profile, session)) {
        Some(Ok(None)) => return None,
        Some(Ok(Some(path))) => Some(path),
        Some(Err(error)) => {
            tracing::error!(%error, "the session keeps its data in memory only");
            None
        }
        None => {
            tracing::error!(%session, "no browser profile; the session keeps its data in memory only");
            None
        }
    };
    if let Some(path) = &path
        && let Err(error) = std::fs::create_dir_all(path)
    {
        tracing::warn!(%error, %session, "the session folder cannot be created");
    }
    let settings = RequestContextSettings {
        cache_path: CefString::from(
            path.as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_str(),
        ),
        // As the global context: kept only when the user said so.
        persist_session_cookies: i32::from(super::persist_session_cookies()),
        ..Default::default()
    };
    let context = request_context_create_context(Some(&settings), None)?;
    CONTEXTS.with(|contexts| {
        contexts
            .borrow_mut()
            .insert(session.to_owned(), context.clone());
    });
    Some(context)
}

/// A request context of its own for one consent probe (RB-094-09, `crate::plugins::consent`): in
/// memory (no cache folder), never kept here, so nothing a probe page sets outlives its browser
/// and no other browser shares it. `None` when CEF makes none - the caller then makes no
/// browser, never one in the global context.
pub fn fresh_request_context() -> Option<RequestContext> {
    let settings = RequestContextSettings {
        cache_path: CefString::from(""),
        persist_session_cookies: 0,
        ..Default::default()
    };
    request_context_create_context(Some(&settings), None)
}

wrap_delete_cookies_callback! {
    // Hands "deleted" to the waiting `done`, once.
    struct Deleted {
        done: Arc<Mutex<Option<CookiesCleared>>>,
    }

    impl DeleteCookiesCallback {
        fn on_complete(&self, _num_deleted: ::std::os::raw::c_int) {
            if let Some(done) = lock(&self.done).take() {
                done(true);
            }
        }
    }
}

/// Deletes every cookie of `session`'s context and calls `done` (`false` when CEF refused).
pub fn clear_cookies(session: &str, done: CookiesCleared) {
    let manager = match request_context(session) {
        Some(context) => context.cookie_manager(None),
        None => cookie_manager_get_global_manager(None),
    };
    let Some(manager) = manager else {
        tracing::warn!(%session, "the session has no cookie manager");
        done(false);
        return;
    };
    let done = Arc::new(Mutex::new(Some(done)));
    let mut callback = Deleted::new(done.clone());
    // An empty URL and name: every cookie of the context.
    if manager.delete_cookies(None, None, Some(&mut callback)) == 0
        && let Some(done) = lock(&done).take()
    {
        done(false);
    }
}

wrap_button_delegate! {
    // The session badge of a cell's label strip: no action of its own.
    struct BadgeDelegate {
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

    impl ButtonDelegate {}
}

/// The label strip of a cell: `label` alone for `default`; beside it, right-aligned, the badge
/// `<dot> <session>` in the session's colour for a named session (in the gap above the view,
/// never over it).
pub fn label_strip(label: &LabelButton, badge: Option<&SessionBadge>, height: u32) -> View {
    let Some(badge) = badge else {
        return View::from(label);
    };
    let tooltip = rb_i18n::t_with("shell.cell.session", &[("session", badge.name.as_str())]);
    let (Some(panel), Some(button)) = (panel_create(None), badge_view(badge, height, &tooltip))
    else {
        return View::from(label);
    };
    let layout = panel.set_to_box_layout(Some(&BoxLayoutSettings {
        horizontal: 1,
        cross_axis_alignment: AxisAlignment::STRETCH,
        ..Default::default()
    }));
    let mut label = View::from(label);
    panel.add_child_view(Some(&mut label));
    let mut button = View::from(&button);
    panel.add_child_view(Some(&mut button));
    if let Some(layout) = &layout {
        layout.set_flex_for_view(Some(&mut label), 1);
        layout.set_flex_for_view(Some(&mut button), 0);
    }
    View::from(&panel)
}

/// The badge in the session's colour, with `tooltip` (`shell.cell.session`).
fn badge_view(badge: &SessionBadge, height: u32, tooltip: &str) -> Option<LabelButton> {
    let mut delegate = BadgeDelegate::new(i32::try_from(height).unwrap_or(i32::MAX));
    let button = label_button_create(
        Some(&mut delegate),
        Some(&CefString::from(badge.text().as_str())),
    )?;
    button.set_enabled_text_colors(argb(badge.rgb));
    button.set_tooltip_text(Some(&CefString::from(tooltip)));
    Some(button)
}
