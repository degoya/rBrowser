//! Copying cookies (RB-040-05): `cookies_copy`, the command `cookies.copy`. The cookies of a
//! Chromium session go into the Firefox and WebKit views of that session; the answer counts per
//! view and names reasons, it never carries a cookie's value.

use rmcp::{handler::server::wrapper::Parameters, schemars, tool, tool_router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::object;
use crate::RbMcpServer;
use crate::answer::{ToolResult, respond};
use crate::backend::ToolFailure;
use crate::target::{Resolved, TargetArg, resolve};

/// The command behind `cookies_copy`.
const COPY: &str = "cookies.copy";

/// Where the cookies come from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum SourceArg {
    /// A view id: that view's session.
    View(u32),
    /// A session id: `default` or one of the project's `sessions`.
    Session(String),
}

/// `cookies_copy`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CookiesCopyParams {
    /// The session to copy from: a session id or a view id (its session). Absent: each Firefox
    /// or WebKit view gets the cookies of its own session.
    #[serde(default)]
    pub source: Option<SourceArg>,
    /// `"all"` (every Firefox and WebKit view, the default), a view id or a breakpoint id.
    #[serde(default)]
    pub target: Option<TargetArg>,
    /// Only the cookies of these sites (host names, a parent domain's cookies included);
    /// absent: the setting `engines.cookies.domainFilter`.
    #[serde(default)]
    pub domains: Option<Vec<String>>,
}

#[tool_router(router = cookies_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Copy the cookies of a Chromium session into the Firefox and WebKit views of that session, so a login in the grid holds there too (each view loads its page again unless engines.cookies.reload is off). source: a session id or a view id (default: each view's own session); target: \"all\" (default), a view id or a breakpoint id; domains: only these sites (default: engines.cookies.domainFilter). Answers per view the cookies copied, refused and skipped with their reasons - never a cookie value; without a Firefox or WebKit view it says so in `message`."
    )]
    pub(crate) async fn cookies_copy(
        &self,
        Parameters(params): Parameters<CookiesCopyParams>,
    ) -> ToolResult {
        respond(self.copy_cookies(params).await)
    }
}

impl RbMcpServer {
    async fn copy_cookies(&self, params: CookiesCopyParams) -> Result<Value, ToolFailure> {
        let target = match params.target {
            None => None,
            Some(target) => {
                let known = self.read(|backend| backend.views()).await?;
                match resolve(Some(&target), &known)? {
                    Resolved::All => None,
                    Resolved::Views(ids) => Some(json!(ids)),
                }
            }
        };
        let args = object([
            ("source", params.source.map(|source| json!(source))),
            ("target", target),
            ("domains", params.domains.map(|domains| json!(domains))),
        ]);
        self.command(COPY, args).await
    }
}

#[cfg(test)]
#[path = "cookies_tests.rs"]
mod tests;
