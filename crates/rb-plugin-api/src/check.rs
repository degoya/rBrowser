//! The native side of the `check-plugin` world: `interface check`.
//!
//! A check plugin describes its rules once and runs the ones switched on per view; its findings
//! speak in one format, so the findings panel, the core MCP tool `checks_run` and `rbrowser check`
//! show those of every check plugin together, merged and deduplicated (RB-060-01).

use crate::{CheckContext, Failure, Finding, PluginHost, RuleInfo};

/// The most findings one `run` may answer; a longer list is refused as a whole.
pub const MAX_FINDINGS: usize = 10_000;

/// The most rules one plugin describes.
pub const MAX_RULES: usize = 256;

/// Whether `id` may name a rule or a shared id: 1 to 64 ASCII letters, digits, `.`, `_` and `-`,
/// starting with a letter or digit - a key of the setting `checks.rules` once the host puts the
/// plugin's id in front.
#[must_use]
pub fn valid_rule_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The logic of a check plugin. A store lives for one call.
pub trait Check {
    /// The rules it offers: id, title key, usual severity, shared id, help page.
    fn describe(&self) -> Vec<RuleInfo>;

    /// Runs the rules `ctx.rules` on `ctx.view` (the plugin's choice for `None`), reporting
    /// progress through [`PluginHost::report_progress`].
    ///
    /// # Errors
    ///
    /// The check failed.
    fn run(
        &mut self,
        host: &mut dyn PluginHost,
        ctx: &CheckContext,
    ) -> Result<Vec<Finding>, Failure>;
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
