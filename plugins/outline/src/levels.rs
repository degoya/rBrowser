//! The person's levels of the outline's rules (`checks.rules.outline.<rule>`, RB-091-06): the one
//! `plugin_util::levels`, shared with `plugins/responsive-images` and `plugins/security-headers`
//! (RB-094-11). `get` - the panel and `outline_get` - shows each finding with the severity the
//! findings run gives it, a rule turned off not at all.

pub use plugin_util::levels::*;

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;
