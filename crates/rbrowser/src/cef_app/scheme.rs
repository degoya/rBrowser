//! `rbrowser://ui/...`: the shell's files from the `ui` folder beside the executable, the
//! placeholder page without a build (see [`crate::shell`]); `rbrowser://plugin/<id>/panel/...`:
//! the plugins' panel files with their CSP (RB-060-12, [`crate::plugins::panels`]);
//! `rbrowser://video/...`: the recorded videos for the Videos panel (RB-110-12,
//! [`super::video_scheme`]); `rbrowser://report/...`: a plugin's HTML report for the report
//! window (RB-091-02, [`super::report_scheme`]).

use std::path::PathBuf;
use std::sync::OnceLock;

use cef::wrapper::stream_resource_handler::StreamResourceHandler;
use cef::*;

use crate::plugins::panels;
use crate::shell::{self, SCHEME, SHELL_HOST, ShellResource};

/// The placeholder page, built once and kept for the process's life (the stream reads it in
/// place).
static PLACEHOLDER: OnceLock<String> = OnceLock::new();

wrap_scheme_handler_factory! {
    struct ShellFiles {
        ui_dir: PathBuf,
    }

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let url = CefString::from(&request?.url()).to_string();
            match shell::resolve(&self.ui_dir, &url) {
                ShellResource::File(path, mime) => {
                    let path = path.to_string_lossy().into_owned();
                    let stream = stream_reader_create_for_file(Some(&CefString::from(path.as_str())))?;
                    Some(StreamResourceHandler::new_with_stream(mime.to_owned(), stream))
                }
                ShellResource::Placeholder => {
                    let page = PLACEHOLDER.get_or_init(shell::placeholder_html);
                    // CEF only reads through the pointer; the page lives as long as the process.
                    let stream = stream_reader_create_for_data(page.as_ptr().cast_mut(), page.len())?;
                    Some(StreamResourceHandler::new_with_stream("text/html".to_owned(), stream))
                }
                ShellResource::NotFound => None,
            }
        }
    }
}

wrap_scheme_handler_factory! {
    struct PluginPanelFiles;

    impl SchemeHandlerFactory {
        fn create(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _scheme_name: Option<&CefString>,
            request: Option<&mut Request>,
        ) -> Option<ResourceHandler> {
            let url = CefString::from(&request?.url()).to_string();
            let file = panels::resource(&url)?;
            // The files live as long as the process (`panels::resource`): CEF reads in place.
            let stream =
                stream_reader_create_for_data(file.bytes.as_ptr().cast_mut(), file.bytes.len())?;
            let mut headers = string_multimap_alloc()?;
            string_multimap_append(
                Some(&mut headers),
                Some(&CefString::from("Content-Security-Policy")),
                Some(&CefString::from(panels::PANEL_CSP_HEADER)),
            );
            Some(StreamResourceHandler::new(
                200,
                "OK".to_owned(),
                file.mime.to_owned(),
                Some(headers),
                Some(stream),
            ))
        }
    }
}

/// Serves the shell from `ui_dir` and the plugins' panel files. Call on the UI thread after CEF
/// is initialised.
pub fn register(ui_dir: PathBuf) {
    let mut factory = ShellFiles::new(ui_dir);
    let registered = register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(SHELL_HOST)),
        Some(&mut factory),
    );
    if registered == 0 {
        tracing::error!("the shell scheme could not be registered");
    }
    let mut panels = PluginPanelFiles::new();
    let registered = register_scheme_handler_factory(
        Some(&CefString::from(SCHEME)),
        Some(&CefString::from(panels::PANEL_HOST)),
        Some(&mut panels),
    );
    if registered == 0 {
        tracing::error!("the plugin panel scheme could not be registered");
    }
    // The Videos panel's recordings (RB-110-12).
    super::video_scheme::register();
    // The report window's reports (RB-091-02).
    super::report_scheme::register();
}
