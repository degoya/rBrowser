//! The helpers rBrowser's first-party plugins carried two or three copies of (RB-094-11, the
//! audit of RB-094-10), once: built into each plugin's wasm32 component as a path dependency, like
//! `plugins/schema-org`, and tested here natively (`scripts/check.sh`, plugin tests).
//!
//! - [`url`]: just enough of an `http(s)` URL for a crawl (`plugins/seo`,
//!   `plugins/sitemap-capture`); `std` only.
//! - [`robots`]: `robots.txt` (RFC 9309) for one product token (the same two); `std` only.
//! - [`base64`]: standard base64 for `data:` URIs (`plugins/pwa-check`, `plugins/seo`); `std`
//!   only.
//! - [`levels`]: the person's levels of a check plugin's own rules (`plugins/outline`,
//!   `plugins/responsive-images`, `plugins/security-headers`); on the plugin contract.
//! - [`mark`]: an element marked in the views over CDP (`plugins/outline`,
//!   `plugins/responsive-images`); on the plugin contract.
//!
//! No platform code and no WASI: `std`, the plugin contract (`rb-plugin-api`, itself built for
//! wasm32) and `serde_json`, which every plugin that uses the last two already carries. A plugin
//! keeps its module path (`crate::url`) as a re-export, so its own code and tests stay as they
//! were.

pub mod base64;
pub mod levels;
pub mod mark;
pub mod robots;
pub mod url;
