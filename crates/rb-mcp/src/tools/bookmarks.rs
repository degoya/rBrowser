//! Project bookmarks (RB-070-06, RB-080-48): `bookmarks_list`, a read the host answers
//! (`Backend::host_tool`) - every bookmark with `relative` (it holds a path that follows the
//! environment) and the URL it leads to now -, and `bookmarks_make_relative`, the command
//! `bookmarks.make_relative`. Both act on the project config, on no view; `navigate` opens a
//! bookmark and `config_set` adds one.

use rmcp::{tool, tool_router};
use serde_json::json;

use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// The host tool behind `bookmarks_list`.
pub const BOOKMARKS_LIST_TOOL: &str = "bookmarks_list";
/// The command behind `bookmarks_make_relative`.
const MAKE_RELATIVE: &str = "bookmarks.make_relative";

#[tool_router(router = bookmarks_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "List the project's bookmarks: for each id, label, group and shortcut if any, relative (true: it stores a path such as /shop/cart?x=1#top and follows the environment; false: an absolute URL, the same everywhere), its path when relative, and url - where it leads in the environment in effect (absent for a path with no environment or start URL to lie under). Also environment (the one in effect), base (the URL paths are put after) and followEnvironment (the setting bookmarks.followEnvironment: new bookmarks of a page under an environment are stored as paths). A read only; acts on no view. Open one with navigate and its url."
    )]
    pub(crate) async fn bookmarks_list(&self) -> ToolResult {
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
        description = "Turn every bookmark with an absolute URL that lies under the project's environments into a path, so it follows the environment from then on (https://staging.example.com/shop/cart becomes /shop/cart). Only a URL whose path leads to the same page under every environment that names its host is converted; a foreign host, or environments with different base paths on that host, keep the URL. Edits only bookmarks[] of config.json and config.local.json (both written or neither), reads the config again and tells the person the number in the status bar. Answers converted (how many), ids, and invalid (bookmarks the config check refuses, left as they are) when there are any. Takes no arguments; acts on no view."
    )]
    pub(crate) async fn bookmarks_make_relative(&self) -> ToolResult {
        respond(self.command(MAKE_RELATIVE, json!({})).await)
    }
}

#[cfg(test)]
#[path = "bookmarks_tests.rs"]
mod tests;
