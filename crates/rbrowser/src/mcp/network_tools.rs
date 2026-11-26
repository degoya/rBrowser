//! The HTTP debugger's MCP tools (RB-080-37), run by the host (`rb_mcp::Backend::host_tool`):
//! `network_breakpoints` (list, add, remove, enable), `network_continue`, `network_abort`,
//! `network_compose` and `network_repeat`. The gates: a continue with edits only while
//! `mcp.allowEvaluate` is on (an edited request or response is content the page gets); a
//! compose or repeat only while `network-panel.allowComposeFromMcp` is on (they send with the
//! person's cookies). Every answer is masked: no cookie, no token. Adding, removing and switching
//! a breakpoint goes through the plugin's own tools, which keep the breakpoints.

use std::sync::Arc;

use rb_engine::ViewId;
use rb_mcp::ToolFailure;
use serde_json::{Value, json};

use crate::control::Core;
use crate::network_rules::compose::ComposeResult;
use crate::network_rules::document::{self, MAX_TIMES};
use crate::network_rules::held::Resolution;
use crate::network_rules::state::Rules;

/// `network_breakpoints`.
pub const BREAKPOINTS_TOOL: &str = "network_breakpoints";
/// `network_continue`.
pub const CONTINUE_TOOL: &str = "network_continue";
/// `network_abort`.
pub const ABORT_TOOL: &str = "network_abort";
/// `network_compose`.
pub const COMPOSE_TOOL: &str = "network_compose";
/// `network_repeat`.
pub const REPEAT_TOOL: &str = "network_repeat";
/// Every tool of this module.
pub const TOOLS: &[&str] = &[
    BREAKPOINTS_TOOL,
    CONTINUE_TOOL,
    ABORT_TOOL,
    COMPOSE_TOOL,
    REPEAT_TOOL,
];
/// The setting that allows `network_compose` and `network_repeat`.
pub const ALLOW_COMPOSE: &str = "network-panel.allowComposeFromMcp";
/// The code while a compose or repeat is off.
pub const COMPOSE_DISABLED: &str = "network.compose_disabled";
/// The code while the debugger does not run (no network-panel plugin).
pub const NOT_RUNNING: &str = "network.not_running";
/// The most bytes of a held body an answer carries.
pub const ANSWER_BODY_BYTES: usize = 32 * 1024;
/// The plugin tools a breakpoint change goes through.
const PLUGIN_BREAKPOINT_TOOLS: [(&str, &str); 3] = [
    ("add", "networkpanel_breakpoint_add"),
    ("remove", "networkpanel_breakpoint_remove"),
    ("enable", "networkpanel_breakpoint_enable"),
];

fn invalid(message: impl Into<String>) -> ToolFailure {
    ToolFailure::new("args.invalid", message)
}

fn rules_of() -> Result<Arc<Rules>, ToolFailure> {
    crate::network_rules::current().ok_or_else(|| {
        ToolFailure::new(
            NOT_RUNNING,
            "the network rules do not run: the network-panel plugin is not loaded",
        )
    })
}

/// Runs tool `name` with `args` in `core`'s window.
///
/// # Errors
///
/// The tool's refusal or failure.
pub async fn host_tool(core: &Arc<Core>, name: &str, args: Value) -> Result<Value, ToolFailure> {
    let rules = rules_of()?;
    match name {
        BREAKPOINTS_TOOL => match args["action"].as_str().unwrap_or("list") {
            "list" => Ok(list(&rules, core.user_settings().mcp.allow_evaluate)),
            action => {
                let Some((_, tool)) = PLUGIN_BREAKPOINT_TOOLS
                    .iter()
                    .find(|(known, _)| *known == action)
                else {
                    return Err(invalid("`action` is list, add, remove or enable"));
                };
                let mut forwarded = args.clone();
                if let Value::Object(map) = &mut forwarded {
                    map.remove("action");
                }
                crate::plugins::mcp::call(core, tool, forwarded)
                    .await
                    .unwrap_or_else(|| {
                        Err(ToolFailure::new(
                            NOT_RUNNING,
                            "the network-panel plugin serves no tools",
                        ))
                    })
            }
        },
        CONTINUE_TOOL => continue_held(&rules, core.user_settings().mcp.allow_evaluate, &args),
        ABORT_TOOL => abort(&rules, &args),
        COMPOSE_TOOL => compose(&rules, &args).await,
        REPEAT_TOOL => repeat(&rules, &args).await,
        other => Err(ToolFailure::new(
            "tool.unknown",
            format!("the host runs no tool `{other}`"),
        )),
    }
}

/// The breakpoints, their switch and the held requests, masked; the held bodies (a login's
/// post data, a token response) only with `bodies` - while `mcp.allowEvaluate` is on.
#[must_use]
pub fn list(rules: &Rules, bodies: bool) -> Value {
    let mut answer = rules.breakpoints_json();
    let mut held = rules.held_masked(ANSWER_BODY_BYTES);
    if !bodies {
        for entry in &mut held {
            if let Value::Object(fields) = entry
                && fields.remove("body").is_some()
            {
                fields.insert("bodyWithheld".to_owned(), Value::Bool(true));
            }
        }
    }
    answer["heldCount"] = json!(held.len());
    answer["held"] = Value::Array(held);
    let settings = rules.settings();
    answer["timeoutMs"] = json!(settings.timeout_ms);
    answer
}

/// The plugin's own tools that reach the debugger, over MCP: `networkpanel_compose` and
/// `networkpanel_repeat` send with the person's cookies and answer only with
/// `network-panel.allowComposeFromMcp`, as the core tools; `networkpanel_debug` carries the held
/// bodies and answers only with `mcp.allowEvaluate` (`network_breakpoints` is the read without
/// them). The panel does not come this way.
///
/// # Errors
///
/// `network.compose_disabled` or `evaluate.disabled`, naming the setting.
pub fn plugin_gate(name: &str, allow_evaluate: bool) -> Result<(), ToolFailure> {
    match name {
        "networkpanel_compose" | "networkpanel_repeat" => {
            compose_allowed(crate::network_rules::current().as_deref())
        }
        "networkpanel_debug" if !allow_evaluate => Err(ToolFailure::new(
            rb_mcp::tools::page::EVALUATE_DISABLED,
            "networkpanel_debug carries the bodies of held requests and is switched off with \
             evaluate; network_breakpoints lists the held requests without them",
        )
        .with_setting(rb_mcp::tools::page::ALLOW_EVALUATE)),
        _ => Ok(()),
    }
}

fn held_id(args: &Value) -> Result<&str, ToolFailure> {
    args["id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128)
        .ok_or_else(|| invalid("`id` names a held request (network_breakpoints lists them)"))
}

fn resolved(
    rules: &Rules,
    id: &str,
    all: bool,
    resolution: Resolution,
) -> Result<Value, ToolFailure> {
    match rules.held().resolve(id, all, resolution) {
        Ok(count) => Ok(json!({ "id": id, "resolved": count, "held": rules.held().len() })),
        Err(error) => Err(ToolFailure::new(
            "network.held_not_found",
            error.to_string(),
        )),
    }
}

/// `network_continue`: `{ id, allViews?, edits? }`; edits only with `allow_evaluate`.
///
/// # Errors
///
/// `args.invalid`, `evaluate.disabled` for edits while evaluate is off, `network.held_not_found`.
pub fn continue_held(
    rules: &Rules,
    allow_evaluate: bool,
    args: &Value,
) -> Result<Value, ToolFailure> {
    let id = held_id(args)?;
    let edits = document::parse_edits(&args["edits"])
        .map_err(|reason| invalid(format!("edits: {reason}")))?;
    let changes = edits.method.is_some()
        || edits.url.is_some()
        || edits.headers.is_some()
        || edits.body.is_some()
        || edits.status.is_some();
    if changes && !allow_evaluate {
        return Err(ToolFailure::new(
            rb_mcp::tools::page::EVALUATE_DISABLED,
            "a continue with edits puts the caller's content into the page and is switched off \
             with evaluate; a person can switch it on in rBrowser at Settings > Security & \
             Privacy > Evaluate (rbrowser://settings/mcp.allowEvaluate)",
        )
        .with_setting(rb_mcp::tools::page::ALLOW_EVALUATE));
    }
    let all = args["allViews"] == Value::Bool(true);
    resolved(rules, id, all, Resolution::Continue(edits))
}

/// `network_abort`: `{ id, allViews? }`.
///
/// # Errors
///
/// `args.invalid`, `network.held_not_found`.
pub fn abort(rules: &Rules, args: &Value) -> Result<Value, ToolFailure> {
    let id = held_id(args)?;
    let all = args["allViews"] == Value::Bool(true);
    resolved(rules, id, all, Resolution::Abort)
}

fn compose_allowed(rules: Option<&Rules>) -> Result<(), ToolFailure> {
    if rules.is_some_and(|rules| rules.settings().allow_compose_from_mcp) {
        return Ok(());
    }
    Err(ToolFailure::new(
        COMPOSE_DISABLED,
        "network_compose and network_repeat send requests with the person's cookies and are \
         switched off; a person can switch them on in rBrowser at Settings > Network > Compose \
         and repeat over MCP (rbrowser://settings/network-panel.allowComposeFromMcp)",
    )
    .with_setting(ALLOW_COMPOSE))
}

fn times(args: &Value) -> Result<u32, ToolFailure> {
    match &args["times"] {
        Value::Null => Ok(1),
        value => value
            .as_u64()
            .filter(|times| (1..=u64::from(MAX_TIMES)).contains(times))
            .and_then(|times| u32::try_from(times).ok())
            .ok_or_else(|| invalid(format!("`times` is 1 to {MAX_TIMES}"))),
    }
}

fn view(args: &Value, key: &str) -> Result<Option<ViewId>, ToolFailure> {
    match &args[key] {
        Value::Null => Ok(None),
        value => value
            .as_u64()
            .and_then(|view| u32::try_from(view).ok())
            .map(|view| Some(ViewId(view)))
            .ok_or_else(|| invalid(format!("`{key}` is a view id"))),
    }
}

fn results_json(results: &[ComposeResult]) -> Value {
    json!({ "results": results.iter().map(|result| result.to_json(true)).collect::<Vec<_>>() })
}

/// `network_compose`: `{ request, view?, times? }` from `view`'s context; off unless
/// `allowComposeFromMcp`.
///
/// # Errors
///
/// `network.compose_disabled`, `args.invalid`, `network.not_running` without a runner.
pub async fn compose(rules: &Rules, args: &Value) -> Result<Value, ToolFailure> {
    compose_allowed(Some(rules))?;
    let request = document::parse_request(&args["request"])
        .map_err(|reason| invalid(format!("request: {reason}")))?;
    let view = view(args, "view")?;
    let times = times(args)?;
    let runner = rules
        .runner()
        .ok_or_else(|| ToolFailure::new(NOT_RUNNING, "no window sends requests"))?;
    let results = runner.compose(view, request, times).await;
    Ok(results_json(&results))
}

/// `network_repeat`: `{ view, requestId, edits?, times? }`; off unless `allowComposeFromMcp`.
///
/// # Errors
///
/// As [`compose`], and `network.request_not_found` for a request the view no longer lists.
pub async fn repeat(rules: &Rules, args: &Value) -> Result<Value, ToolFailure> {
    compose_allowed(Some(rules))?;
    let view = view(args, "view")?.ok_or_else(|| invalid("`view` is required"))?;
    let request_id = args["requestId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 200)
        .ok_or_else(|| {
            invalid("`requestId` names a captured request (network_requests, networkpanel_list)")
        })?
        .to_owned();
    let edits = document::parse_edits(&args["edits"])
        .map_err(|reason| invalid(format!("edits: {reason}")))?;
    let times = times(args)?;
    let runner = rules
        .runner()
        .ok_or_else(|| ToolFailure::new(NOT_RUNNING, "no window sends requests"))?;
    let results = runner
        .repeat(view, request_id, edits, times)
        .await
        .map_err(|reason| ToolFailure::new("network.request_not_found", reason))?;
    Ok(results_json(&results))
}

#[cfg(test)]
#[path = "network_tools_tests.rs"]
mod tests;
