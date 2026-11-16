//! `rbrowser://report/<token>/<file>`: the report window's reports (RB-091-02). Which file an
//! address names, and whether it may be served, is [`crate::report_view::resolve`]'s - a live
//! token, the open project still the token's and trusted, and the path guard, checked again on
//! every load; this is CEF's side. The factory answers only the main frame of a report window's
//! browser ([`crate::report_view::windows::requester_allowed`], the ids `report_window.rs`
//! registers): a grid view, a popup, the shell, a panel or a frame in the report gets no handler
//! and so no answer, whatever address it names. The file is read no further than
//! [`MAX_REPORT_BYTES`] and served whole, once, with the report's Content-Security-Policy
//! ([`CSP`]: no network, no frames, no forms). A refusal is logged with its code alone, never a
//! path or the address (its token opens the report).

use std::sync::{Arc, Mutex};

use cef::*;

use crate::cdp::lock;
use crate::report_view::guard::{MAX_REPORT_BYTES, read_bounded};
use crate::report_view::tokens::{CSP, ROUTE_HOST};
use crate::report_view::windows::requester_allowed;
use crate::shell::SCHEME;

/// The most bytes handed over at once.
const CHUNK: usize = 256 * 1024;

/// The headers of every report.
const HEADERS: [(&str, &str); 5] = [
    ("Content-Security-Policy", CSP),
    ("X-Content-Type-Options", "nosniff"),
    ("Referrer-Policy", "no-referrer"),
    ("Cache-Control", "no-store"),
    ("Cross-Origin-Opener-Policy", "same-origin"),
];

/// A report read into memory, and how much of it went out.
struct Body {
    bytes: Vec<u8>,
    sent: usize,
}

/// Copies `bytes` into CEF's read buffer.
#[allow(
    unsafe_code,
    reason = "CEF hands the read's buffer as a raw pointer; no safe wrapper copies into it"
)]
fn copy_out(bytes: &[u8], data_out: *mut u8) {
    // SAFETY: CEF's `read` passes `data_out` with at least `bytes_to_read` writable bytes, valid
    // for the call; `bytes` is at most that long (the read asked for no more) and is a buffer of
    // our own, so the two never overlap.
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), data_out, bytes.len());
    }
}

wrap_resource_handler! {
    struct ReportAnswer {
        body: Arc<Mutex<Option<Body>>>,
    }

    impl ResourceHandler {
        fn open(
            &self,
            request: Option<&mut Request>,
            handle_request: Option<&mut i32>,
            _callback: Option<&mut Callback>,
        ) -> i32 {
            if let Some(handle_request) = handle_request {
                *handle_request = 1;
            }
            let Some(request) = request else {
                return 0;
            };
            let url = CefString::from(&request.url()).to_string();
            let host = super::views::core().map(|core| core.host().clone());
            let (root, trusted) = host.map_or((None, false), |host| {
                (host.project_root(), host.shell_state().trusted)
            });
            let bytes = crate::report_view::resolve(&url, root, trusted)
                .and_then(|path| read_bounded(&path, MAX_REPORT_BYTES));
            match bytes {
                Ok(bytes) => {
                    *lock(&*self.body) = Some(Body { bytes, sent: 0 });
                    1
                }
                Err(refusal) => {
                    tracing::info!(code = refusal.code(), "a report request was refused");
                    0
                }
            }
        }

        fn response_headers(
            &self,
            response: Option<&mut Response>,
            response_length: Option<&mut i64>,
            _redirect_url: Option<&mut CefString>,
        ) {
            let body = lock(&*self.body);
            let length = body.as_ref().map_or(0, |body| body.bytes.len());
            if let Some(response) = response {
                if body.is_some() {
                    response.set_status(200);
                    response.set_status_text(Some(&CefString::from("OK")));
                    response.set_mime_type(Some(&CefString::from("text/html")));
                    for (name, value) in HEADERS {
                        response.set_header_by_name(
                            Some(&CefString::from(name)),
                            Some(&CefString::from(value)),
                            1,
                        );
                    }
                } else {
                    response.set_status(404);
                }
            }
            if let Some(response_length) = response_length {
                *response_length = i64::try_from(length).unwrap_or(-1);
            }
        }

        fn read(
            &self,
            data_out: *mut u8,
            bytes_to_read: i32,
            bytes_read: Option<&mut i32>,
            _callback: Option<&mut ResourceReadCallback>,
        ) -> i32 {
            let Some(bytes_read) = bytes_read else {
                return 0;
            };
            *bytes_read = 0;
            let wanted = usize::try_from(bytes_to_read).unwrap_or(0).min(CHUNK);
            if wanted == 0 || data_out.is_null() {
                return 0;
            }
            let mut body = lock(&*self.body);
            let Some(body) = body.as_mut() else {
                return 0;
            };
            let rest = &body.bytes[body.sent.min(body.bytes.len())..];
            if rest.is_empty() {
                // The answer is complete.
                return 0;
            }
            let count = rest.len().min(wanted);
            copy_out(&rest[..count], data_out);
            body.sent += count;
            *bytes_read = i32::try_from(count).unwrap_or(0);
            1
        }

        fn cancel(&self) {
            *lock(&*self.body) = None;
        }
    }
}

wrap_scheme_handler_factory! {
    struct ReportFiles;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            _request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            // Only a report window's main frame: no other browser, no frame in a page, no request
            // without one.
            let browser = browser.map(|browser| browser.identifier());
            let main = frame.is_some_and(|frame| frame.is_main() != 0);
            if !requester_allowed(browser, main) {
                tracing::info!(main, "a report request from outside a report window was refused");
                return None;
            }
            Some(ReportAnswer::new(Arc::new(Mutex::new(None))))
        }
    }
}

/// Serves `rbrowser://report/`. Call on the UI thread after CEF is initialised.
pub fn register() {
    let mut factory = ReportFiles::new();
    let registered = register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(ROUTE_HOST)),
        Some(&mut factory),
    );
    if registered == 0 {
        tracing::error!("the report route could not be registered");
    }
}
