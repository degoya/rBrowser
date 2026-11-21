//! The person's levels of the header rules (`checks.rules.security-headers.<rule>`, RB-091-06):
//! the one `plugin_util::levels`, shared with `plugins/outline` and `plugins/responsive-images`
//! (RB-094-11). The command `check`, the MCP tools and the panel give each finding the severity
//! the findings run gives it, a rule turned off not at all.

pub use plugin_util::levels::*;

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;
