//! The client of a window a grid page opened (`window.open`, `target=_blank`): only the guard
//! that keeps the shell's pages out of it. UI thread.

use std::os::raw::c_int;

use cef::*;

use crate::shell::{PageOwner, navigation_refused};

fn text(value: &CefStringUserfree) -> String {
    CefString::from(value).to_string()
}

wrap_client! {
    pub(super) struct PopupClient {}

    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(PopupRequests::new())
        }
    }
}

wrap_request_handler! {
    struct PopupRequests {}

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: c_int,
            _is_redirect: c_int,
        ) -> c_int {
            let url = request.map(|request| text(&request.url())).unwrap_or_default();
            let main = frame.as_ref().is_some_and(|frame| frame.is_main() != 0);
            if navigation_refused(PageOwner::Popup, main, &url) {
                tracing::warn!("a popup's navigation to a shell page was refused");
                return 1;
            }
            0
        }
    }
}
