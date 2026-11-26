//! The MCP tool annotations of every core tool (RB-099-16): `readOnlyHint`, `destructiveHint`,
//! `idempotentHint` and `openWorldHint`, so a client tells `views_list` from `session_clear`.
//! One function derives them, [`of`], from what the coverage tables already say and four short
//! lists; [`annotate`] puts them on the router's tools. A hint is left out only where MCP's
//! default says the same or the spec calls it meaningless, so `tools/list` stays small: a read
//! says `readOnlyHint: true` and `openWorldHint`; any other tool `destructiveHint` and
//! `openWorldHint` (both default to true), and `idempotentHint` when it is true. Hints only -
//! every guard stays in the tool.

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::ToolAnnotations;

use crate::RbMcpServer;
use crate::coverage::tables;

/// Tools that change nothing beyond those [`tables::WITHOUT_COMMAND`] names "a read". A `clear`
/// or `reset` of the MCP's own log buffers and counters is not a change of the pages or the
/// project.
pub const READS: &[&str] = &[
    "app_info",
    "app_perf",
    "breakpoints_detect",
    "cdp_endpoint",
    "env_compare_result",
    "flow_list",
    "ios_devices",
    "plugins_catalog_list",
    "plugins_list",
    "portal_devices",
    "settings_get",
    "sitemap_discover",
    "sitemap_status",
];

/// Tools that remove, overwrite or reset what was there: data, files, config, views - and the
/// ones that install code (an extension, an update that replaces the program), so a client asks
/// before it lets an agent run them.
pub const DESTRUCTIVE: &[&str] = &[
    "bookmarks_make_relative",
    "breakpoint_remove",
    "breakpoint_update",
    "breakpoints_apply",
    "config_set",
    "cookies_copy",
    "evaluate",
    "extension_install",
    "extension_remove",
    "layout_save_preset",
    "network_abort",
    "portal_disconnect",
    "recorder_step_remove",
    "recorder_step_update",
    "script_run",
    "session_assign",
    "session_clear",
    "settings_set",
    "update_install",
    "view_remove",
    "visual_baseline_update",
];

/// Tools besides the reads and the `*_set` tools that a second call with the same arguments
/// leaves as the first did.
pub const IDEMPOTENT: &[&str] = &[
    "breakpoint_remove",
    "breakpoint_update",
    "canvas_place",
    "canvas_zoom",
    "checks_cancel",
    "env_compare_cancel",
    "extension_remove",
    "panels_dock",
    "portal_disconnect",
    "portal_present_stop",
    "portal_stop",
    "session_clear",
    "sitemap_cancel",
    "view_page_zoom",
    "view_pop_in",
    "view_pop_out",
    "view_set_breakpoint",
    "views_set_engine",
];

/// Tools that reach beyond rBrowser and the open project: they load pages or send requests
/// (sites, environments, the update host, the Web Store), or run code that may.
pub const OPEN_WORLD: &[&str] = &[
    "back",
    "breakpoints_detect",
    "checks_run",
    "compare_engines",
    "env_compare",
    "env_compare_live",
    "evaluate",
    "extension_install",
    "flow_run",
    "forward",
    "navigate",
    "network_compose",
    "network_continue",
    "network_repeat",
    "plugins_catalog_list",
    "recorder_play",
    "recorder_step_try",
    "reload",
    "script_run",
    "sitemap_capture",
    "sitemap_discover",
    "update_check",
    "update_install",
    "views_restore_last",
];

/// Whether `tool` only reads: listed in [`READS`], or a tool no command runs because it is "a
/// read" ([`tables::WITHOUT_COMMAND`]).
#[must_use]
pub fn is_read(tool: &str) -> bool {
    READS.contains(&tool)
        || tables::without_command(tool).is_some_and(|reason| reason.starts_with("a read"))
}

/// What the four hints of the core tool `tool` mean, each spelled out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hints {
    /// It changes nothing.
    pub read_only: bool,
    /// It removes, overwrites or resets what was there.
    pub destructive: bool,
    /// A second call with the same arguments changes nothing more.
    pub idempotent: bool,
    /// It reaches beyond rBrowser and the open project.
    pub open_world: bool,
}

/// The hints of the core tool `tool`.
#[must_use]
pub fn hints(tool: &str) -> Hints {
    let read_only = is_read(tool);
    Hints {
        read_only,
        destructive: !read_only && DESTRUCTIVE.contains(&tool),
        idempotent: read_only || tool.ends_with("_set") || IDEMPOTENT.contains(&tool),
        open_world: OPEN_WORLD.contains(&tool),
    }
}

/// The annotations of the core tool `tool`: [`hints`] without what MCP's defaults already say.
#[must_use]
pub fn of(tool: &str) -> ToolAnnotations {
    let hints = hints(tool);
    if hints.read_only {
        return ToolAnnotations::from_raw(None, Some(true), None, None, Some(hints.open_world));
    }
    ToolAnnotations::from_raw(
        None,
        None,
        Some(hints.destructive),
        hints.idempotent.then_some(true),
        Some(hints.open_world),
    )
}

/// Puts [`of`] on every tool of `router`.
pub fn annotate(router: &mut ToolRouter<RbMcpServer>) {
    for (name, route) in &mut router.map {
        route.attr.annotations = Some(of(name.as_ref()));
    }
}

/// Every tool the lists name, with the list: what the tests hold to the registered tools.
#[must_use]
pub fn named() -> Vec<(&'static str, &'static str)> {
    let lists: [(&str, &[&str]); 4] = [
        ("READS", READS),
        ("DESTRUCTIVE", DESTRUCTIVE),
        ("IDEMPOTENT", IDEMPOTENT),
        ("OPEN_WORLD", OPEN_WORLD),
    ];
    lists
        .into_iter()
        .flat_map(|(list, tools)| tools.iter().map(move |tool| (list, *tool)))
        .collect()
}

#[cfg(test)]
#[path = "annotations_tests.rs"]
mod tests;
