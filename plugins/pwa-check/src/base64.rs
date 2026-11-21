//! Standard base64 with padding (RFC 4648), for the icons the panel shows as `data:` URIs - the
//! panel's frame has no network. The one `plugin_util::base64`, shared with `plugins/seo`
//! (RB-094-11).

pub use plugin_util::base64::*;

#[cfg(test)]
#[path = "base64_tests.rs"]
mod tests;
