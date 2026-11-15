//! The shell view's content: the built Vue app (`ui/dist`, shipped as `ui\` beside the
//! executable) served under the app's own scheme, `rbrowser://ui/` (RB-010-07 builds it).
//! Without a build the shell shows a placeholder page, so the grid works on its own.
//!
//! Only pages of [`SHELL_ORIGIN`] in the shell view get `window.cefQuery`; a grid view can
//! neither load the scheme ([`crate::host::nav::normalize_url`]) nor reach the core.

use std::path::{Path, PathBuf};

use rb_plugin_types::html::escape;

/// The app's scheme.
pub const SCHEME: &str = "rbrowser";
/// The host of the shell's pages.
pub const SHELL_HOST: &str = "ui";
/// Every shell page starts with this.
pub const SHELL_ORIGIN: &str = "rbrowser://ui/";
/// The shell's entry page.
pub const SHELL_URL: &str = "rbrowser://ui/index.html";
/// The folder beside the executable that holds the built shell.
pub const UI_DIR: &str = "ui";
/// The height of the shell view in DIP (the toolbar).
pub const SHELL_HEIGHT: u32 = 44;
/// The status bar's page at the bottom of the main window (RB-080-15).
pub const STATUS_BAR_URL: &str = "rbrowser://ui/status-bar.html";
/// The status bar's height in DIP: one line of small text.
pub const STATUS_BAR_HEIGHT: u32 = 24;

/// The entry page of the pages window (RB-010-08, contract 0.1-w1b): one window of its own
/// beside the main one, for everything larger than the toolbar - the settings, the trust dialog
/// (RB-010-09), the analyser's wizard (RB-010-13). The shell view never grows over the grid.
pub const PAGES_URL: &str = "rbrowser://ui/settings.html";
/// The route of the settings; `settings/<key>` opens them at a key (the deep link
/// `rbrowser://settings/<key>`).
pub const ROUTE_SETTINGS: &str = "settings";
/// The route of the trust dialog (RB-010-09).
pub const ROUTE_TRUST: &str = "trust";
/// The route of the breakpoint analyser's wizard (RB-010-13).
pub const ROUTE_ANALYZER: &str = "analyzer";

/// The pages window's URL for `route` (`settings/sync.scroll`, `trust`): the route is the
/// hash, so the window's app routes without a server.
pub fn page_url(route: &str) -> String {
    format!("{PAGES_URL}#/{route}")
}

/// Whether `url` is a page of the shell.
pub fn is_shell_url(url: &str) -> bool {
    url.get(..SHELL_ORIGIN.len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case(SHELL_ORIGIN))
}

/// Which browser a navigation happens in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageOwner {
    /// The shell view.
    Shell,
    /// A view of the grid.
    View,
    /// A window a grid page opened (`window.open`, `target=_blank`): no grid view, never
    /// reported or synchronised.
    Popup,
}

/// Whether a navigation of `owner` to `url` (in the main frame when `main`) is refused: the
/// shell stays on its own pages, and no grid view or popup ever loads one.
pub fn navigation_refused(owner: PageOwner, main: bool, url: &str) -> bool {
    match owner {
        PageOwner::Shell => main && !is_shell_url(url),
        PageOwner::View | PageOwner::Popup => is_shell_url(url),
    }
}

/// What a request for a shell URL is answered with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ShellResource {
    /// A file of the built shell, with its MIME type.
    File(PathBuf, &'static str),
    /// The placeholder page (no build present).
    Placeholder,
    /// Nothing: outside the shell, a path that escapes its folder, a missing file.
    NotFound,
}

/// Maps a request for `url` to a file under `ui_dir`. The path is percent-decoded; `..`,
/// backslashes, drive letters and empty segments are refused, so nothing outside `ui_dir` is
/// ever read.
pub fn resolve(ui_dir: &Path, url: &str) -> ShellResource {
    if !is_shell_url(url) {
        return ShellResource::NotFound;
    }
    let rest = &url[SHELL_ORIGIN.len()..];
    let path = rest.split(['?', '#']).next().unwrap_or_default();
    let Some(path) = percent_decode(path) else {
        return ShellResource::NotFound;
    };
    let path = if path.is_empty() {
        "index.html".to_owned()
    } else {
        path
    };
    let segments: Vec<&str> = path.split('/').collect();
    let safe = segments.iter().all(|segment| {
        !segment.is_empty()
            && *segment != "."
            && *segment != ".."
            && !segment.contains(['\\', ':', '\0'])
    });
    if !safe {
        return ShellResource::NotFound;
    }
    let file = segments
        .iter()
        .fold(ui_dir.to_path_buf(), |dir, segment| dir.join(segment));
    if file.is_file() {
        ShellResource::File(file.clone(), mime_type(&file))
    } else if path == "index.html" {
        ShellResource::Placeholder
    } else {
        ShellResource::NotFound
    }
}

fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = text.get(index + 1..index + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// The MIME type of a shell file by its extension.
pub fn mime_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match extension.as_str() {
        "html" | "htm" => "text/html",
        "js" | "mjs" => "text/javascript",
        "css" => "text/css",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "wasm" => "application/wasm",
        "txt" => "text/plain",
        _ => "application/octet-stream",
    }
}

/// The placeholder page: a dark toolbar strip that says the shell is not built.
pub fn placeholder_html() -> String {
    let title = escape(&rb_i18n::t("shell.placeholder.title"));
    let hint = escape(&rb_i18n::t("shell.placeholder.hint"));
    format!(
        "<!doctype html><html lang=\"en\"><meta charset=\"utf-8\"><title>{title}</title>\
         <style>html,body{{margin:0;height:100%;background:#1f2430;color:#d8dbe2;\
         font:13px system-ui,sans-serif}}body{{display:flex;align-items:center;gap:12px;\
         padding:0 12px;box-sizing:border-box}}b{{font-weight:600}}</style>\
         <b>{title}</b><span>{hint}</span></html>"
    )
}

/// The most characters of a shell page's console line the log keeps.
pub const CONSOLE_MAX_CHARS: usize = 2000;

/// The log level of a shell page's console line by CEF's `cef_log_severity_t` value: errors
/// and warnings stay visible at the default level, `console.log` is debug, verbose is trace.
pub fn console_level(severity: i32) -> tracing::Level {
    match severity {
        4 | 5 => tracing::Level::ERROR,
        3 => tracing::Level::WARN,
        1 => tracing::Level::TRACE,
        _ => tracing::Level::DEBUG,
    }
}

/// A console line for the log, cut after [`CONSOLE_MAX_CHARS`] characters.
pub fn console_text(message: &str) -> String {
    match message.char_indices().nth(CONSOLE_MAX_CHARS) {
        Some((end, _)) => format!("{}...", &message[..end]),
        None => message.to_owned(),
    }
}

/// Writes a shell page's console line (`source`:`line`) into the app's log, so a shell that
/// does not come up says why (CEF's own log keeps console lines only at `info`).
pub fn log_console(severity: i32, message: &str, source: &str, line: i32) {
    let message = console_text(message);
    match console_level(severity) {
        tracing::Level::ERROR => tracing::error!(source, line, "shell console: {message}"),
        tracing::Level::WARN => tracing::warn!(source, line, "shell console: {message}"),
        tracing::Level::TRACE => tracing::trace!(source, line, "shell console: {message}"),
        _ => tracing::debug!(source, line, "shell console: {message}"),
    }
}

/// Whether a page of the shell at Chromium's zoom `level` is zoomed (RB-080-50): the shell's
/// pages never zoom, so any level but 0 is put back when a page of the shell has loaded
/// (`cef_app/zoom.rs`). Chromium keeps the zoom per host and across restarts - a level left by
/// an earlier run counts too.
pub fn is_zoomed(level: f64) -> bool {
    level.is_nan() || level.abs() >= 1e-6
}

/// CEF's `EVENTFLAG_CONTROL_DOWN` and `EVENTFLAG_ALT_DOWN` and `EVENTFLAG_SHIFT_DOWN`.
const FLAG_SHIFT: u32 = 1 << 1;
const FLAG_CONTROL: u32 = 1 << 2;
const FLAG_ALT: u32 = 1 << 3;

/// Whether a key-down with CEF's event `flags` and the Windows key code `key` is one Chromium
/// zooms a page with: Ctrl with Plus, Minus (the main keys and the number pad's) or 0 (never
/// with Shift, which is rBrowser's own), and no Alt (AltGr). A page of the shell never takes
/// these (RB-080-50, `cef_app/keys.rs`).
pub fn is_zoom_key(flags: u32, key: i32) -> bool {
    /// `VK_OEM_PLUS`, `VK_ADD`, `VK_OEM_MINUS`, `VK_SUBTRACT`.
    const STEP_KEYS: [i32; 4] = [0xBB, 0x6B, 0xBD, 0x6D];
    /// `0`, `VK_NUMPAD0`.
    const RESET_KEYS: [i32; 2] = [0x30, 0x60];
    if flags & FLAG_CONTROL == 0 || flags & FLAG_ALT != 0 {
        return false;
    }
    STEP_KEYS.contains(&key) || (flags & FLAG_SHIFT == 0 && RESET_KEYS.contains(&key))
}

#[cfg(test)]
#[path = "shell_tests.rs"]
mod tests;
