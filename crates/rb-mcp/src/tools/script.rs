//! `script_run` (RB-098-01, MCP code mode): a short JavaScript calls the tools itself and only its
//! return value comes back - one call instead of a round trip per tool and view. Off until a
//! person switches on `mcp.scripts.enabled`; the run, its bridge and its answer are
//! [`crate::scripts`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, schemars, tool, tool_router};
use serde::Deserialize;
use serde_json::Value;

use crate::RbMcpServer;
use crate::answer::ToolResult;
use crate::scripts::answer::{Ran, answer, refused};
use crate::scripts::bridge::{Bridge, RouterDispatch};
use crate::scripts::{
    ENABLED_KEY, MAX_ARGS_BYTES, MAX_CODE_BYTES, ScriptRequest, ScriptStop, ScriptTools, codes,
};
use crate::target::TargetArg;

/// How much longer than `mcp.scripts.timeoutMs` the server waits for the sandbox to stop on
/// its own before it gives up on the run.
const STOP_GRACE: Duration = Duration::from_secs(2);

/// `script_run`.
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScriptRunParams {
    /// The script: the body of an async function with the parameters `args` and `target`;
    /// `return` gives the answer. At most 64 KiB.
    pub code: String,
    /// Any JSON value; the script gets it as `args` (default `{}`).
    #[serde(default)]
    pub args: Option<Value>,
    /// `"all"` (default), a view id or a breakpoint id; the script gets it as `target` and
    /// passes it on to the tools it calls.
    #[serde(default)]
    pub target: Option<TargetArg>,
}

#[tool_router(router = script_router, vis = "pub(crate)")]
impl RbMcpServer {
    #[tool(
        description = "Run a short JavaScript that calls rBrowser's tools itself and answers only what it returns (MCP code mode): one call instead of a round trip per tool and view, the intermediate results never enter the context. code is the body of an async function with the parameters args (the JSON given as args) and target (as given, default \"all\"). Inside, await rb.<tool>(arguments) runs that MCP tool with exactly the guards of a direct call and gives its JSON answer; a refusal throws an Error named ToolError with code and tool. console.log lines come back as logs. Answer: result (the returned value as JSON), calls, durationMs, cut at mcp.scripts.maxResultBytes. The script runs in a sandbox: no fetch, require, import, timers, file system, network or process; at most mcp.scripts.maxCalls tool calls within mcp.scripts.timeoutMs. Off by default: a person switches it on with the setting mcp.scripts.enabled (code scripts_disabled). Failure codes: script.syntax (with line), script.timeout, script.call_limit, script.recursion (script_run inside a script), script.memory, script.stack, script.error or the tool's own code for an uncaught error (with name, tool, line). Example: await rb.navigate({ target, url: args.url }); const views = await rb.evaluate({ target, expression: 'document.documentElement.scrollWidth > innerWidth' }); return views.filter((v) => v.value).map((v) => v.breakpoint);"
    )]
    pub(crate) async fn script_run(
        &self,
        Parameters(params): Parameters<ScriptRunParams>,
        context: RequestContext<RoleServer>,
    ) -> ToolResult {
        Ok(self.run_script(params, context).await)
    }
}

impl RbMcpServer {
    async fn run_script(
        &self,
        params: ScriptRunParams,
        context: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let options = self.backend.options().scripts;
        if !options.enabled {
            return refused(
                codes::DISABLED,
                "MCP code mode is switched off; a person can switch it on in rBrowser at \
                 Settings > Security & Privacy > Evaluate (rbrowser://settings/mcp.scripts.enabled)",
                Some(ENABLED_KEY),
            );
        }
        if params.code.len() > MAX_CODE_BYTES {
            return refused(
                "invalid_arguments",
                &format!("code is longer than {MAX_CODE_BYTES} bytes"),
                None,
            );
        }
        let args = params
            .args
            .unwrap_or_else(|| Value::Object(serde_json::Map::new()))
            .to_string();
        if args.len() > MAX_ARGS_BYTES {
            return refused(
                "invalid_arguments",
                &format!("args is longer than {MAX_ARGS_BYTES} bytes as JSON"),
                None,
            );
        }
        let target = params.target.unwrap_or_else(TargetArg::all);
        let target = serde_json::to_string(&target).unwrap_or_else(|_| "\"all\"".to_owned());
        let Some(runner) = self.backend.script_runner() else {
            return refused(
                codes::UNAVAILABLE,
                "this rBrowser has no script engine",
                None,
            );
        };
        let bridge = Arc::new(Bridge::new(
            RouterDispatch::new(self.clone(), context),
            options.max_calls,
        ));
        let tools: Arc<dyn ScriptTools> = bridge.clone();
        let request = ScriptRequest {
            code: params.code,
            args,
            target,
            timeout: options.timeout,
        };
        let started = Instant::now();
        let outcome =
            match tokio::time::timeout(options.timeout + STOP_GRACE, runner.run(request, tools))
                .await
            {
                Ok(outcome) => outcome,
                Err(_) => Err(ScriptStop::Timeout),
            };
        let ran = Ran {
            calls: bridge.calls(),
            duration: started.elapsed(),
            exceeded: bridge.exceeded(),
        };
        answer(outcome, ran, &options)
    }
}
