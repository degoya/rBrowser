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
        description = "The Chrome extensions of the Chromium views: enabled (engines.chromium.extensions.enabled), each unpacked folder of the person's list and each extension installed from the Chrome Web Store (source webStore) with its name, version, id, whether this start loaded it or why it is refused, restartNeeded when the set changed since the start, webStore (engines.chromium.extensions.allowWebStore), and a trusted project's recommendations (folders inside the project; a Web Store entry with installable true can be installed with extension_install). A read: no tool loads or removes an unpacked folder - that is the person's command in rBrowser (Extensions: load an unpacked extension, Ctrl+Shift+Space X O); a Web Store extension is removed with extension_remove after the person's word."
    )]
    pub(crate) async fn extension_list(&self) -> ToolResult {
        let result = async {
            let answer: Value = self.backend.host_tool(LIST_TOOL, json!({})).await?;
            Ok(self.backend.mask(&answer))
        }
        .await;
        respond(result)
    }

    #[tool(
        description = "Install an extension from the Chrome Web Store (id: the extension id or its store page URL). rBrowser downloads it from Google's update server, checks its CRX3 signatures (the developer's key is the id, the Web Store's key the publisher) and then asks the person in its window; only their Install unpacks it, and it loads into every Chromium view at the next start (restartNeeded true). Declined or not answered within ten minutes: code extensions.declined. Off while engines.chromium.extensions.allowWebStore is off (code extensions.web_store_off, settingsKey). No tool grants the confirmation."
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
        description = "Remove an extension installed from the Chrome Web Store (id: the extension id or its store page URL). rBrowser names it in its window and deletes its folder under the local store (every version, the pointer, staging leftovers) only on the person's Remove; what this start loaded stays loaded until the next start (restartNeeded true then). Not installed from the store: code extensions.not_installed; declined or not answered within ten minutes: code extensions.declined. Off while engines.chromium.extensions.allowWebStore is off (code extensions.web_store_off, settingsKey). Unpacked folders are never removed. No tool grants the confirmation."
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
