//! The person's levels of the check's rules (`checks.rules.responsive-images.<rule>`,
//! RB-091-06): the one `plugin_util::levels`, shared with `plugins/outline` and
//! `plugins/security-headers` (RB-094-11). The tool `check` - the panel and `images_check` - shows
//! each finding with the severity the findings run gives it, a rule turned off not at all.

pub use plugin_util::levels::*;

#[cfg(test)]
#[path = "levels_tests.rs"]
mod tests;
