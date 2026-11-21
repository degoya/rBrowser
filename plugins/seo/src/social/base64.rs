//! Standard base64 with padding (RFC 4648), for the share image the panel shows as a `data:` URI -
//! the panel's frame has no network. The one `plugin_util::base64`, shared with
//! `plugins/pwa-check` (RB-094-11).

pub use plugin_util::base64::*;

#[cfg(test)]
#[path = "base64_tests.rs"]
mod tests;
