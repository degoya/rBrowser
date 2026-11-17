//! `views` of the user settings (RB-090-20, Layout > Zoom): the page zoom inside a view -
//! Chromium's own zoom of the page, not the grid's zoom of the cell. Read by
//! `crates/rbrowser/src/page_zoom`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `views`: what every view does on its own.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewsSettings {
    /// The page zoom inside a view (RB-090-20).
    pub page_zoom: PageZoomSettings,
}

/// `views.pageZoom`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct PageZoomSettings {
    /// With the focus in a page, Ctrl+0 sets the view's page zoom back to 100 % and Ctrl+Plus
    /// and Ctrl+Minus zoom it in Chromium's steps, as in Chrome. Off, the keys reach the page;
    /// the commands and the badge in the label strip stay.
    pub keys: bool,
    /// Ctrl and the mouse wheel over a page zoom it, as in Chromium. Off, rBrowser puts a
    /// zoom the wheel made back at once; only the keys, the commands and MCP zoom.
    pub wheel: bool,
    /// A page zoom applies to every view at once, wherever it was made. Off, it is the view's
    /// own - except that Chromium keeps the zoom per site: views of one session showing the
    /// same host zoom together, and each one's badge says so.
    pub sync: bool,
    /// A zoomed page keeps its zoom when it reloads or navigates, as Chromium keeps it per
    /// site. Off, every load starts at 100 %.
    pub keep_on_reload: bool,
}

impl Default for PageZoomSettings {
    fn default() -> Self {
        Self {
            keys: true,
            wheel: true,
            sync: false,
            keep_on_reload: true,
        }
    }
}

#[cfg(test)]
#[path = "page_zoom_settings_tests.rs"]
mod tests;
