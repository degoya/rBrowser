//! The CEF application object, the same in every process: the app's scheme (all processes),
//! the browser's command-line switches (CDP pipe, host resolver, proxy), the browser process
//! handler and the renderer's handler.

use std::sync::OnceLock;

use cef::*;

use super::render::ShellRenderer;
use super::start::Startup;
use crate::shell::SCHEME;

/// The switches the browser process adds to its own command line: name and optional value.
pub static SWITCHES: OnceLock<Vec<(String, Option<String>)>> = OnceLock::new();

wrap_app! {
    pub struct RbApp {}

    impl App {
        fn on_before_command_line_processing(
            &self,
            process_type: Option<&CefString>,
            command_line: Option<&mut CommandLine>,
        ) {
            let browser = process_type.is_none_or(|kind| kind.to_string().is_empty());
            let Some(line) = command_line else {
                return;
            };
            if !browser {
                return;
            }
            for (name, value) in SWITCHES.get().into_iter().flatten() {
                let name = CefString::from(name.as_str());
                match value {
                    Some(value) => line.append_switch_with_value(
                        Some(&name),
                        Some(&CefString::from(value.as_str())),
                    ),
                    None => line.append_switch(Some(&name)),
                }
            }
        }

        fn on_register_custom_schemes(&self, registrar: Option<&mut SchemeRegistrar>) {
            let Some(registrar) = registrar else {
                return;
            };
            use cef::sys::cef_scheme_options_t as option;
            let options = option::CEF_SCHEME_OPTION_STANDARD as i32
                | option::CEF_SCHEME_OPTION_SECURE as i32
                | option::CEF_SCHEME_OPTION_CORS_ENABLED as i32
                | option::CEF_SCHEME_OPTION_FETCH_ENABLED as i32;
            if registrar.add_custom_scheme(Some(&CefString::from(SCHEME)), options) == 0 {
                tracing::error!("the app's scheme could not be registered");
            }
        }

        fn browser_process_handler(&self) -> Option<BrowserProcessHandler> {
            Some(Startup::new())
        }

        fn render_process_handler(&self) -> Option<RenderProcessHandler> {
            Some(ShellRenderer::new())
        }
    }
}
