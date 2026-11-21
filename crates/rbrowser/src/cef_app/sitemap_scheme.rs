//! `rbrowser://sitemap/<run>/<slug>/<breakpoint>.png`: the shots the sitemap gallery page shows
//! (RB-090-86). Which file an address names, and whether it may be served, is
//! [`crate::sitemap::gallery_page::open_shot`]'s - three plain names, a shot of the run the
//! gallery window shows ([`super::gallery_window::shown_run`]), walked from the open project
//! without a link, at most `MAX_GALLERY_SHOT_BYTES` - checked on every request; this is CEF's
//! side. `open` only opens the file; `read` streams it in chunks of at most [`CHUNK`], so no
//! shot is held whole and no thread reads one in one go (code review, wave 090-w22). The
//! factory answers only the main frame of the gallery window's browser: a grid view, a popup,
//! the shell, the pages window, a panel or a frame gets no handler and so no answer, whatever
//! address it names. A shot is served as `image/png`, read-only. A refusal is logged with
//! its code alone, never a path or the address.

use std::io::Read as _;
use std::sync::{Arc, Mutex};

use cef::*;
use rb_visual::sitemap_store::ShotFile;

use crate::cdp::lock;
use crate::shell::SCHEME;
use crate::sitemap::gallery_page::{ROUTE_HOST, open_shot, requester_allowed};

/// The most bytes handed over at once.
const CHUNK: usize = 256 * 1024;

/// The headers of every shot.
const HEADERS: [(&str, &str); 3] = [
    ("X-Content-Type-Options", "nosniff"),
    ("Cache-Control", "no-store"),
    ("Content-Security-Policy", "default-src 'none'"),
];

/// A shot opened for the answer, and how much of it went out.
struct Body {
    shot: ShotFile,
    sent: u64,
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
    struct ShotAnswer {
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
            let root = super::views::core().and_then(|core| core.host().project_root());
            let shown = super::gallery_window::shown_run();
            match open_shot(root.as_deref(), shown.as_deref(), &url) {
                Ok(shot) => {
                    *lock(&*self.body) = Some(Body { shot, sent: 0 });
                    1
                }
                Err(refusal) => {
                    tracing::info!(code = refusal.code(), "a gallery shot was refused");
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
            let length = body.as_ref().map_or(0, |body| body.shot.length);
            if let Some(response) = response {
                if body.is_some() {
                    response.set_status(200);
                    response.set_status_text(Some(&CefString::from("OK")));
                    response.set_mime_type(Some(&CefString::from("image/png")));
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
            let left = body.shot.length.saturating_sub(body.sent);
            let wanted = usize::try_from(left).map_or(wanted, |left| left.min(wanted));
            if wanted == 0 {
                // The answer is complete.
                return 0;
            }
            let mut chunk = vec![0_u8; wanted];
            let count = match body.shot.file.read(&mut chunk) {
                Ok(count) => count,
                Err(error) => {
                    tracing::info!(kind = ?error.kind(), "a gallery shot could not be read on");
                    return 0;
                }
            };
            if count == 0 {
                // The file got shorter since it was opened: what is there went out.
                return 0;
            }
            copy_out(&chunk[..count], data_out);
            body.sent += count as u64;
            *bytes_read = i32::try_from(count).unwrap_or(0);
            1
        }

        fn cancel(&self) {
            *lock(&*self.body) = None;
        }
    }
}

wrap_scheme_handler_factory! {
    struct GalleryShots;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            _request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            // Only the gallery window's main frame: no other browser, no frame in a page, no
            // request without one.
            let gallery = super::gallery_window::browser_id();
            let browser = browser.map_or(0, |browser| browser.identifier());
            let main = frame.is_some_and(|frame| frame.is_main() != 0);
            if !requester_allowed(gallery, browser, main) {
                tracing::info!(main, "a gallery shot from outside the gallery window was refused");
                return None;
            }
            Some(ShotAnswer::new(Arc::new(Mutex::new(None))))
        }
    }
}

/// Serves `rbrowser://sitemap/`. Call on the UI thread after CEF is initialised.
pub fn register() {
    let mut factory = GalleryShots::new();
    let registered = register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(ROUTE_HOST)),
        Some(&mut factory),
    );
    if registered == 0 {
        tracing::error!("the gallery's shot route could not be registered");
    }
}
