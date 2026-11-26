//! The plugin catalogue (RB-091-01): `plugins_catalog_list`, the read of the commands
//! `plugins.catalog` and `plugins.check_updates`. It lists what the configured repositories
//! offer and which installed plugins have an update, and fetches the repositories again with
//! `refresh`; **it installs nothing and adds no repository** - both are a person's, in
//! Settings > Plugins > Catalog (`omitted("protected: plugin install/trust")`).

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::Value;

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// `plugins_catalog_list`.
#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogParams {
    /// Fetch every configured repository's signed index again first (network); default false,
    /// the indexes rBrowser holds.
    #[serde(default)]
    pub refresh: Option<bool>,
}

#[tool_router(router = plugin_catalogue_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "List the plugin catalogue: the repositories with their state, every plugin they offer (origin, version, capabilities, publisher, whether it runs on this rBrowser), the updates of plugins installed from it, and the installed plugins. refresh fetches the signed indexes first. Installing a plugin or adding a repository is a person's decision, never MCP's."
    )]
    pub(crate) async fn plugins_catalog_list(
        &self,
        Parameters(params): Parameters<CatalogParams>,
    ) -> ToolResult {
        // Named arguments (an object, never `null`): the core answers with the catalogue and
        // opens no window for an agent's call.
        let args = object([("refresh", params.refresh.map(Value::Bool))]);
        respond(self.command("plugins.catalog", args).await)
    }
}

#[cfg(test)]
#[path = "plugin_catalogue_tests.rs"]
mod tests;
