//! `sitemap` of the user settings (RB-090-83): where a sitemap capture's gallery opens - the
//! person's own choice, never a project's. A checked-in config could otherwise send a gallery a
//! repository put into `.rbrowser/out/` to the system browser, where it runs with script on a
//! `file:` address (security review, wave 090-w21); the key is protected from MCP too
//! (`annotate::USER_KEYS`). Re-exported from the crate root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Where a sitemap capture's gallery opens: never in the grid's views.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum GalleryOpensIn {
    /// A window of rBrowser's own.
    #[default]
    Window,
    /// The system's default browser.
    System,
}

/// `sitemap`: Capture > Sitemap, the person's part.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct UserSitemapSettings {
    /// Where the gallery of a capture opens: a window of rBrowser's own or the system browser.
    pub gallery_opens_in: GalleryOpensIn,
}

#[cfg(test)]
#[path = "user_sitemap_tests.rs"]
mod tests;
