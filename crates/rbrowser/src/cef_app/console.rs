//! The console of the shell's own pages (the shell view, the pages window, the page windows) in
//! the app's log ([`crate::shell::log_console`]): CEF's log keeps console lines only at `info`,
//! so without this a shell that fails at start leaves no trace. UI thread.

use std::os::raw::c_int;

use cef::*;

wrap_display_handler! {
    pub(super) struct ShellConsole {}

    impl DisplayHandler {
        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            level: LogSeverity,
            message: Option<&CefString>,
            source: Option<&CefString>,
            line: c_int,
        ) -> c_int {
            let text = |value: Option<&CefString>| value.map(ToString::to_string).unwrap_or_default();
            crate::shell::log_console(i64::from(level.get_raw()), &text(message), &text(source), line);
            // CEF goes on as before (its own log at `info`).
            0
        }
    }
}
