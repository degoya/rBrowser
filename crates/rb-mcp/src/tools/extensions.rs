//! Chrome extensions (RB-080-06, RB-080-23, RB-080-30): `extension_list`, `extension_install`
//! and `extension_remove`, all run by the host (`crates/rbrowser/src/extensions/`). The list is
//! a read: the person's unpacked folders and the installed Web Store extensions, whether this
//! start loaded each, whether a restart is needed, and a trusted project's recommendations. The
//! install downloads and checks an extension from the Chrome Web Store and then waits for the
//! person's word in rBrowser's window; without it nothing is unpacked. The remove names an
//! installed Web Store extension in the window and deletes it only on the person's word. No
//! tool loads or removes an unpacked folder - that is the person's command in rBrowser.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::NoArgs;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};

/// The host tool behind the list.
const LIST_TOOL: &str = "extension_list";
/// The host tool behind the install.
const INSTALL_TOOL: &str = "extension_install";
/// The host tool behind the removal.
const REMOVE_TOOL: &str = "extension_remove";

/// `extension_install`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionInstallParams {
    /// The extension id (32 letters a to p) or its Chrome Web Store page URL.
    pub id: String,
}

/// `extension_remove`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExtensionRemoveParams {
    /// The extension id (32 letters a to p) or its Chrome Web Store page URL.
    pub id: String,
}

#[tool_router(router = extensions_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "The Chrome extensions of the Chromium views: enabled (engines.chromium.extensions.enabled), each unpacked folder of the person's list and each Web Store extension (source webStore) with name, version, id, whether this start loaded it or why it is refused, restartNeeded, webStore (engines.chromium.extensions.allowWebStore), and a trusted project's recommendations (installable true: extension_install takes it). Loading or removing an unpacked folder is the person's command, never a tool's."
    )]
    pub(crate) async fn extension_list(&self, _args: Parameters<NoArgs>) -> ToolResult {
        let result = async {
            let answer: Value = self.backend.host_tool(LIST_TOOL, json!({})).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Install an extension from the Chrome Web Store (id: the extension id or its store URL). rBrowser downloads it, checks its CRX3 signatures and asks the person in its window; only their Install unpacks it, and it loads at the next start (restartNeeded). Declined or no answer within ten minutes: extensions.declined. Off while engines.chromium.extensions.allowWebStore is off (extensions.web_store_off)."
    )]
    pub(crate) async fn extension_install(
        &self,
        Parameters(params): Parameters<ExtensionInstallParams>,
    ) -> ToolResult {
        let result = async {
            let answer: Value = self
                .backend
                .host_tool(INSTALL_TOOL, json!({ "id": params.id }))
                .await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Remove an extension installed from the Chrome Web Store (id: the extension id or its store URL). rBrowser deletes it only on the person's Remove in its window (extensions.declined otherwise, also after ten minutes); what this start loaded stays until the next start (restartNeeded). Not from the store: extensions.not_installed. Off while engines.chromium.extensions.allowWebStore is off (extensions.web_store_off). Unpacked folders are never removed."
    )]
    pub(crate) async fn extension_remove(
        &self,
        Parameters(params): Parameters<ExtensionRemoveParams>,
    ) -> ToolResult {
        let result = async {
            let answer: Value = self
                .backend
                .host_tool(REMOVE_TOOL, json!({ "id": params.id }))
                .await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }
}

#[cfg(test)]
#[path = "extensions_tests.rs"]
mod tests;
