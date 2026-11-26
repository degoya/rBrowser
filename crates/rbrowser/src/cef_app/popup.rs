//! The client of a window a grid page opened (`window.open`, `target=_blank`): the guard that
//! keeps the shell's pages out of it - the render views' browsers take the same guard
//! (`render_views.rs`) - and its Basic auth challenges (review w35): the stored credential, else
//! rBrowser's sign-in bar naming the view that opened it; a render view is never asked. UI
//! thread, the challenges on the IO thread.

use std::os::raw::c_int;

use cef::*;
use rb_engine::ViewId;

use crate::basic_auth::later::Requester;
use crate::shell::{PageOwner, navigation_refused};

fn text(value: &CefStringUserfree) -> String {
    CefString::from(value).to_string()
}

wrap_client! {
    pub(super) struct PopupClient {
        opener: ViewId,
    }

    impl Client {
        fn request_handler(&self) -> Option<RequestHandler> {
            Some(PopupRequests::new(Requester::Popup(self.opener)))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(PopupLifeSpan::new())
        }
    }
}

wrap_life_span_handler! {
    struct PopupLifeSpan {}

    impl LifeSpanHandler {
        // Its requests waiting in rBrowser's sign-in bar end with it.
        fn on_before_close(&self, browser: Option<&mut Browser>) {
            super::auth::view_closed(browser.as_deref());
        }
    }
}

wrap_request_handler! {
    pub(super) struct PopupRequests {
        requester: Requester,
    }

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

        // A Basic auth challenge: the stored credential, else the bar - never for a render view.
        fn auth_credentials(
            &self,
            browser: Option<&mut Browser>,
            origin_url: Option<&CefString>,
            is_proxy: c_int,
            host: Option<&CefString>,
            port: c_int,
            realm: Option<&CefString>,
            scheme: Option<&CefString>,
            callback: Option<&mut AuthCallback>,
        ) -> c_int {
            let asked = super::auth::Asked { origin_url, is_proxy, host, port, realm, scheme };
            super::auth::answer(Some(self.requester), browser, &asked, callback)
        }
    }
}
