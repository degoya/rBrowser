//! Project bookmarks (RB-070-06, RB-080-48): `bookmarks_list`, a read the host answers
//! (`Backend::host_tool`) - every bookmark with `relative` (it holds a path that follows the
//! environment) and the URL it leads to now -, and `bookmarks_make_relative`, the command
//! `bookmarks.make_relative`. Both act on the project config, on no view; `navigate` opens a
//! bookmark and `config_set` adds one.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};
use serde_json::json;

use super::NoArgs;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// The host tool behind `bookmarks_list`.
pub const BOOKMARKS_LIST_TOOL: &str = "bookmarks_list";
/// The command behind `bookmarks_make_relative`.
const MAKE_RELATIVE: &str = "bookmarks.make_relative";

#[tool_router(router = bookmarks_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "List the project's bookmarks: id, label, group, shortcut, relative (true: a path such as /shop/cart that follows the environment; false: an absolute URL), path, and url - where it leads in the environment in effect. Also environment, base and followEnvironment (bookmarks.followEnvironment). Open one with navigate and its url."
    )]
    pub(crate) async fn bookmarks_list(&self, _args: Parameters<NoArgs>) -> ToolResult {
        let result = async {
            let answer = self
                .backend
                .host_tool(BOOKMARKS_LIST_TOOL, json!({}))
                .await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Turn every bookmark whose absolute URL lies under the project's environments into a path, so it follows the environment (https://staging.example.com/shop/cart becomes /shop/cart). A foreign host, or environments with different base paths on that host, keep the URL. Edits only bookmarks[] of config.json and config.local.json (both or neither). Answers converted, ids and invalid (bookmarks the config check refuses, left as they are)."
    )]
    pub(crate) async fn bookmarks_make_relative(&self, _args: Parameters<NoArgs>) -> ToolResult {
        respond(self.command(MAKE_RELATIVE, json!({})).await)
    }
}

#[cfg(test)]
#[path = "bookmarks_tests.rs"]
mod tests;
