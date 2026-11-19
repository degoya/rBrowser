//! The MCP server: one [`RbMcpServer`] per session over a shared [`Backend`]. The tools live in
//! `tools/<area>.rs`, each a thin layer over a command of the one registry or a read of the host.

use std::sync::Arc;

use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{Implementation, ServerCapabilities, ServerConfig},
    tool_handler,
};
use serde_json::Value;

use crate::backend::{Backend, ToolFailure};

/// What the server tells an agent at the start, kept beside the code; `server_tests` holds the
/// tools it names against [`RbMcpServer::tool_names`].
pub const INSTRUCTIONS: &str = include_str!("instructions.md");

/// One MCP session's server.
#[derive(Clone)]
pub struct RbMcpServer {
    pub(crate) backend: Arc<dyn Backend>,
    pub(crate) tool_router: ToolRouter<Self>,
}

impl RbMcpServer {
    /// A server over `backend` with every tool.
    #[must_use]
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        // The loaded plugins' tools beside the core ones (RB-060-12).
        let plugins = crate::tools::plugins::router(backend.plugin_tools());
        Self {
            backend,
            tool_router: Self::router() + plugins,
        }
    }

    /// Every tool this server offers.
    pub(crate) fn router() -> ToolRouter<Self> {
        Self::views_router()
            + Self::views_edit_router()
            + Self::project_router()
            + Self::layout_router()
            + Self::app_router()
            + Self::page_router()
            + Self::logs_router()
            + Self::config_router()
            + Self::sync_router()
            + Self::capture_router()
            + Self::cdp_router()
            + Self::flows_router()
            + Self::engines_router()
            + Self::cookies_router()
            + Self::recorder_router()
            + Self::visual_router()
            + Self::sitemap_router()
            + Self::env_compare_router()
            + Self::metrics_router()
            + Self::video_router()
            + Self::status_router()
            + Self::extensions_router()
            + Self::network_debug_router()
            + Self::portal_router()
            + Self::devtools_router()
            + Self::bookmarks_router()
            + Self::panels_router()
            + Self::plugin_catalogue_router()
            + Self::page_zoom_router()
            + Self::force_state_router()
            + Self::updates_router()
    }

    /// The names of every tool, sorted: what the coverage test holds the commands against.
    #[must_use]
    pub fn tool_names() -> Vec<String> {
        let mut names: Vec<String> = Self::router()
            .list_all()
            .into_iter()
            .map(|tool| tool.name.into_owned())
            .collect();
        names.sort();
        names
    }

    /// Runs the command `id` of the registry off the async workers.
    pub(crate) async fn command(&self, id: &str, args: Value) -> Result<Value, ToolFailure> {
        let backend = Arc::clone(&self.backend);
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || backend.run_command(&id, args))
            .await
            .map_err(|_| ToolFailure::new("internal", "the command did not finish"))?
            .map_err(ToolFailure::from)
    }

    /// Runs `read` on the backend off the async workers.
    pub(crate) async fn read<T: Send + 'static>(
        &self,
        read: impl FnOnce(&dyn Backend) -> T + Send + 'static,
    ) -> Result<T, ToolFailure> {
        let backend = Arc::clone(&self.backend);
        tokio::task::spawn_blocking(move || read(backend.as_ref()))
            .await
            .map_err(|_| ToolFailure::new("internal", "the read did not finish"))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RbMcpServer {
    fn get_info(&self) -> ServerConfig {
        let mut info = ServerConfig::new(
            ServerCapabilities::builder()
                .enable_tools()
                // Sent by `rbrowser mcp` when plugin tools come or go (`http::TOOLS_REVISION`).
                .enable_tool_list_changed()
                .build(),
        )
        .with_instructions(INSTRUCTIONS);
        let mut implementation = Implementation::default();
        implementation.name = "rbrowser".to_owned();
        implementation.title = Some("rBrowser".to_owned());
        implementation.version = env!("CARGO_PKG_VERSION").to_owned();
        info.server_info = implementation;
        info
    }
}

#[cfg(test)]
#[path = "server_tests.rs"]
mod tests;
