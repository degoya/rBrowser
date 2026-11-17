//! The splitter between the grid and docked DevTools (RB-010-06), CEF side: a thin Alloy
//! browser view showing [`crate::devtools::splitter::page_url`], [`THICKNESS`] DIP across the
//! dock's axis, between the grid and the dock (`devtools.rs` lays it out). The page's title is
//! the drag ([`crate::devtools::splitter::on_title`]); the view loads nothing but its page and
//! opens no popups. CEF Views panels get no mouse events, hence a browser view. UI thread.

use std::os::raw::c_int;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use cef::*;

use super::devtools::{Extent, NoPopups};
use super::views::core;
use crate::devtools::splitter::{THICKNESS, may_load, on_title, page_url};

/// The splitter's browser view; its thickness follows the dock's side in `extent`.
pub(super) fn view(extent: Arc<Extent>) -> Option<BrowserView> {
    let mut client = SplitterClient::new();
    let mut delegate = SplitterDelegate::new(extent);
    browser_view_create(
        Some(&mut client),
        Some(&CefString::from(page_url().as_str())),
        Some(&BrowserSettings::default()),
        None,
        None,
        Some(&mut delegate),
    )
}

wrap_browser_view_delegate! {
    // The splitter: Alloy style like the dock, thin across the dock's axis, stretched along it.
    struct SplitterDelegate {
        extent: Arc<Extent>,
    }

    impl ViewDelegate {
        fn preferred_size(&self, _view: Option<&mut View>) -> Size {
            let thickness = i32::try_from(THICKNESS).unwrap_or(6);
            if self.extent.right.load(Ordering::Relaxed) {
                super::views::strip(thickness, 0)
            } else {
                super::views::strip(0, thickness)
            }
        }
    }

    impl BrowserViewDelegate {
        fn browser_runtime_style(&self) -> RuntimeStyle {
            RuntimeStyle::ALLOY
        }
    }
}

wrap_client! {
    // The splitter's handlers: its title, the guard on what it loads, no popups.
    struct SplitterClient {}

    impl Client {
        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(SplitterTitle::new())
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(SplitterRequests::new())
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(NoPopups::new())
        }
    }
}

wrap_display_handler! {
    struct SplitterTitle {}

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            if let (Some(title), Some(core)) = (title, core()) {
                on_title(core, &title.to_string());
            }
        }
    }
}

wrap_request_handler! {
    struct SplitterRequests {}

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            _user_gesture: c_int,
            _is_redirect: c_int,
        ) -> c_int {
            let main = frame.as_ref().is_some_and(|frame| frame.is_main() != 0);
            let url = request
                .map(|request| CefString::from(&request.url()).to_string())
                .unwrap_or_default();
            if main && !may_load(&url) {
                tracing::warn!("the DevTools splitter refused to leave its page");
                return 1;
            }
            0
        }
    }
}
