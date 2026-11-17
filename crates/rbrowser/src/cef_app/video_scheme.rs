//! `rbrowser://video/<recording>/<file>`: the recorded videos of the open project for the Videos
//! panel (RB-110-12). The checks and the bytes are [`crate::video::serve`]'s; this is CEF's side:
//! the factory refuses every frame but a shell page's main frame - a page in a view, a frame in
//! it or a plugin panel gets no handler and so no answer - and the handler answers with the
//! status, the range headers and the bytes [`Served`] gives, read on CEF's worker thread - only
//! of a recording rBrowser wrote (`crate::video::registry`, parsed again when it changes).
//! Every refusal is logged with its reason - the code, the recording and the file name, never a
//! path or a page's address - so a diagnostics ZIP says why a video did not load.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use cef::*;

use crate::cdp::lock;
use crate::shell::SCHEME;
use crate::video::registry::Registry;
use crate::video::serve::{
    ERR_FAILED, ROUTE_HOST, Served, logged_range, parse_url, requester_allowed, skip_answer,
};

/// Whether a request from outside the shell's pages was logged at `info` already: the first one
/// goes to the log the diagnostics carry, the rest only to `debug` - a page cannot fill the log.
static OUTSIDER_LOGGED: AtomicBool = AtomicBool::new(false);

/// The recording and file a refused request named, for the log: never a path.
fn named(url: &str) -> (String, String) {
    parse_url(url).unwrap_or_else(|| ("-".to_owned(), "-".to_owned()))
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
    struct VideoAnswer {
        root: PathBuf,
        registry: PathBuf,
        served: Arc<Mutex<Option<Served>>>,
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
            let range = CefString::from(&request.header_by_name(Some(&CefString::from("Range"))))
                .to_string();
            let range = (!range.is_empty()).then_some(range);
            // Parsed again only when the registry file changed: a recording deleted since is no
            // longer served; the folder walk and the file's checks run on every request.
            let recorded = Registry::for_project(&self.registry, &self.root).load_cached();
            match Served::open(&self.root, &recorded, &url, range.as_deref()) {
                Ok(served) => {
                    // Every answer, so a diagnostics ZIP shows the player's requests: the status,
                    // the range asked, the range answered and the length - never a path.
                    let (recording, file) = named(&url);
                    tracing::info!(
                        %recording,
                        %file,
                        status = served.status().0,
                        range = %logged_range(range.as_deref()),
                        content_range = served.content_range().as_deref().unwrap_or("-"),
                        length = served.length(),
                        mime = served.mime(),
                        "a video request was answered"
                    );
                    *lock(&*self.served) = Some(served);
                    1
                }
                Err(error) => {
                    // The reason's code and the names only: the error's text may hold a path.
                    let (recording, file) = named(&url);
                    tracing::info!(
                        code = error.code(),
                        %recording,
                        %file,
                        known = recorded.knows(&recording),
                        "a video request was refused"
                    );
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
            let served = lock(&*self.served);
            let Some(served) = served.as_ref() else {
                if let Some(response) = response {
                    response.set_status(404);
                }
                if let Some(length) = response_length {
                    *length = 0;
                }
                return;
            };
            if let Some(response) = response {
                let (status, text) = served.status();
                response.set_status(status);
                response.set_status_text(Some(&CefString::from(text)));
                response.set_mime_type(Some(&CefString::from(served.mime())));
                for (name, value) in served.headers() {
                    response.set_header_by_name(
                        Some(&CefString::from(name)),
                        Some(&CefString::from(value.as_str())),
                        1,
                    );
                }
            }
            if let Some(length) = response_length {
                *length = i64::try_from(served.length()).unwrap_or(-1);
            }
        }

        fn skip(
            &self,
            bytes_to_skip: i64,
            bytes_skipped: Option<&mut i64>,
            _callback: Option<&mut ResourceSkipCallback>,
        ) -> i32 {
            let Some(bytes_skipped) = bytes_skipped else {
                return 0;
            };
            let mut served = lock(&*self.served);
            let skipped = match (served.as_mut(), u64::try_from(bytes_to_skip)) {
                (Some(served), Ok(count)) => served.skip(count).ok(),
                _ => None,
            };
            let (answer, count) = skip_answer(bytes_to_skip, skipped);
            *bytes_skipped = count;
            answer
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
            if bytes_to_read <= 0 || data_out.is_null() {
                return 0;
            }
            let mut served = lock(&*self.served);
            let Some(served) = served.as_mut() else {
                return 0;
            };
            // At most `bytes_to_read`, so the copy fits CEF's buffer.
            match served.read_for_loader(bytes_to_read) {
                // The answer is complete.
                Ok(bytes) if bytes.is_empty() => 0,
                Ok(bytes) => {
                    copy_out(&bytes, data_out);
                    *bytes_read = i32::try_from(bytes.len()).unwrap_or(0);
                    1
                }
                Err(error) => {
                    tracing::warn!(kind = ?error.kind(), "a video could not be read");
                    *bytes_read = ERR_FAILED;
                    0
                }
            }
        }

        fn cancel(&self) {
            // Closes the file: a recording the player let go of can be deleted.
            if lock(&*self.served).take().is_some() {
                tracing::debug!("a video request was cancelled");
            }
        }
    }
}

wrap_scheme_handler_factory! {
    struct VideoFiles;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            _request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let Some(frame) = frame else {
                tracing::info!("a video request without a frame was refused");
                return None;
            };
            let url = CefString::from(&frame.url()).to_string();
            let main = frame.is_main() != 0;
            if !requester_allowed(Some(&url), main) {
                // Whether it was a main frame and a shell page - never the page's address.
                let shell = crate::shell::is_shell_url(&url);
                if OUTSIDER_LOGGED.swap(true, Ordering::Relaxed) {
                    tracing::debug!(main, shell, "a video request from another page was refused");
                } else {
                    tracing::info!(main, shell, "a video request from another page was refused");
                }
                return None;
            }
            let root = super::views::core().and_then(|core| core.host().project_root());
            let Some(root) = root else {
                tracing::info!("a video request was refused: no project is open");
                return None;
            };
            let Some(registry) = crate::video::registry::default_dir() else {
                tracing::info!("a video request was refused: no local data folder");
                return None;
            };
            Some(VideoAnswer::new(root, registry, Arc::new(Mutex::new(None))))
        }
    }
}

/// Serves `rbrowser://video/`. Call on the UI thread after CEF is initialised.
pub fn register() {
    let mut factory = VideoFiles::new();
    let registered = register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(ROUTE_HOST)),
        Some(&mut factory),
    );
    if registered == 0 {
        tracing::error!("the video route could not be registered");
    }
}
