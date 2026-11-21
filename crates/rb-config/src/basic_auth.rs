//! `basicAuth`: Project > Environments (RB-094-08) - which environments sit behind HTTP Basic
//! auth. The section names environments only: the user and password of each one live in the
//! platform's credential store (Windows Credential Manager), bound to the project and the
//! environment's origin, and never in `config.json` or `config.local.json` - a section that
//! carries any other field does not load. The key is `x-protected`: MCP neither reads the
//! credentials nor turns the answer on for another environment. Read by
//! `crates/rbrowser/src/basic_auth/`. Re-exported from the crate root.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `basicAuth`: the environments whose Basic auth challenge rBrowser answers (RB-094-08).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct BasicAuthConfig {
    /// The ids of the environments behind Basic auth. Their credentials are set with
    /// `environment.auth` and kept in the credential store, never here. The settings window
    /// offers the project's environments (`x-options`).
    #[schemars(extend("x-options" = "environments"))]
    pub environments: Vec<String>,
}

impl BasicAuthConfig {
    /// Whether the environment `id` is behind Basic auth.
    #[must_use]
    pub fn uses(&self, id: &str) -> bool {
        self.environments.iter().any(|listed| listed == id)
    }
}

#[cfg(test)]
#[path = "basic_auth_tests.rs"]
mod tests;
