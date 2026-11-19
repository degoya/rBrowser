//! What the tool definitions cannot say themselves, kept beside them and held to them by the
//! coverage test ([`super::tools::tool_problems`]): the tools no command runs, the tools that act
//! on no view, the ones that act on exactly one, and the guarded ones. A table naming a tool
//! that is not registered is a problem, so none of them can go stale.

use crate::tools::page::ALLOW_EVALUATE;

/// The tools no command of the registry runs, and why: reads of the host, and the edits the
/// settings window makes key by key.
pub const WITHOUT_COMMAND: &[(&str, &str)] = &[
    (
        "portal_stop",
        "stops the portal; portal.toggle starts it too, which no tool may (a LAN listener is a \
         human's decision)",
    ),
    (
        "breakpoint_remove",
        "edits the project config as the settings window does; no command removes a breakpoint",
    ),
    (
        "breakpoint_update",
        "edits the project config as the settings window does; no command changes a breakpoint",
    ),
    (
        "bus_send",
        "puts a message on the page API's bus from outside; no command sends one",
    ),
    (
        "click",
        "replays a click into the views over CDP; no command clicks an element",
    ),
    ("config_get", "a read: the effective project config"),
    ("console_logs", "a read: the console buffer of the views"),
    (
        "hover",
        "replays the mouse onto an element over CDP; no command hovers an element",
    ),
    ("errors", "a read: the error buffer of the views"),
    (
        "extension_list",
        "a read: the Chrome extensions; adding one is the person's command, never MCP's",
    ),
    (
        "extension_install",
        "downloads and checks a Web Store extension, then waits for the person's word in the \
         window; the person's own command extensions.install_web_store asks nothing",
    ),
    (
        "evaluate",
        "runs page JavaScript over CDP; no command runs a script",
    ),
    (
        "network_requests",
        "a read: the network buffer of the views",
    ),
    (
        "network_breakpoints",
        "the HTTP debugger's breakpoints and held requests (RB-080-37); the person's command \
         networkpanel_breakpoints_toggle switches them, adding one is the panel's form",
    ),
    (
        "network_continue",
        "lets a held request go on over CDP; no command continues one by id",
    ),
    (
        "network_abort",
        "fails a held request over CDP; no command aborts one by id",
    ),
    (
        "network_compose",
        "sends a request of the caller's own from the core; no command composes one",
    ),
    (
        "network_repeat",
        "sends a captured request again from the core; no command repeats one by id",
    ),
    (
        "press_key",
        "replays a key into the views over CDP; no command presses a key in a page",
    ),
    ("project_info", "a read: the open project"),
    (
        "scroll",
        "scrolls the views as scroll sync does; no command scrolls a page",
    ),
    ("snapshot", "a read: the accessibility tree of the views"),
    (
        "type",
        "replays typed text into the views over CDP; no command types text",
    ),
    ("views_list", "a read: the views of the grid"),
    (
        "view_metrics",
        "a read: the Web Vitals and resources of the views",
    ),
    (
        "status_messages",
        "a read: the status bar's message history",
    ),
    (
        "engines_list",
        "a read: the installed Firefox editions; the path is the person's setting",
    ),
    (
        "bookmarks_list",
        "a read: the project's bookmarks, each with relative and the URL it leads to now",
    ),
    (
        "video_list",
        "a read: the recorded videos; capture.video.panel shows them to the person",
    ),
    (
        "update_status",
        "a read: the self-update's state; update.check and update.install are the commands",
    ),
];

/// The tools that act on no view and so take no `target`, with what they act on instead.
pub const UNTARGETED: &[(&str, &str)] = &[
    ("app_info", "the app"),
    ("breakpoint_add", "the project config"),
    ("breakpoint_remove", "the project config"),
    ("breakpoint_update", "the project config"),
    ("breakpoints_apply", "the project config"),
    ("breakpoints_detect", "the site's stylesheets"),
    ("config_get", "the project config"),
    ("config_set", "the project config"),
    ("diagnostics", "the app"),
    ("engines_list", "the installed Firefox editions"),
    ("env_compare_cancel", "an environment comparison run"),
    ("env_compare_result", "an environment comparison run"),
    ("environment_set", "the project's environments"),
    ("extension_install", "the Chrome extensions of the instance"),
    ("extension_list", "the Chrome extensions of the instance"),
    ("extension_remove", "the Chrome extensions of the instance"),
    ("flow_list", "the project's flows"),
    ("layout_save_preset", "the project config"),
    ("layout_set", "the grid"),
    (
        "network_abort",
        "one held request of the HTTP debugger, by id",
    ),
    (
        "network_breakpoints",
        "the HTTP debugger's breakpoints over every view (views names them)",
    ),
    (
        "network_compose",
        "the core's HTTP client (view names the context)",
    ),
    (
        "network_continue",
        "one held request of the HTTP debugger, by id",
    ),
    (
        "network_repeat",
        "the core's HTTP client (view names the captured request)",
    ),
    ("plugins_catalog_list", "the plugin catalogue"),
    ("plugins_list", "the plugins"),
    ("portal_devices", "the portal's paired devices"),
    ("portal_disconnect", "the portal's paired devices"),
    ("portal_stop", "the portal's LAN listener"),
    ("project_info", "the project"),
    ("project_open", "the recent projects"),
    ("recorder_export", "the stored recordings"),
    ("recorder_stop", "the window's recorder"),
    ("session_clear", "a session's cookies and storage"),
    ("settings_get", "the settings"),
    ("settings_set", "the settings"),
    ("sitemap_cancel", "a sitemap capture run"),
    (
        "sitemap_capture",
        "hidden render views of its own, per breakpoint",
    ),
    ("sitemap_gallery", "a sitemap capture run's gallery window"),
    ("sitemap_status", "a sitemap capture run"),
    ("status_messages", "the window's message history"),
    ("view_add", "the hidden selection of the views"),
    (
        "view_add_size",
        "the project config and the hidden selection of the views",
    ),
    ("views_list", "the grid"),
    ("wsl_info", "WSL"),
    ("bookmarks_list", "the project's bookmarks"),
    ("bookmarks_make_relative", "the project config"),
    ("panels_dock", "the window's plugin panels"),
    ("checks_cancel", "the check run in progress"),
    ("canvas_zoom", "the canvas camera"),
    ("video_list", "the project's recorded videos"),
    ("update_check", "the app"),
    ("update_install", "the app"),
    ("update_status", "the app"),
];

/// The view tools that act on exactly one view: their `target` is a view id or a breakpoint id
/// shown once, the active view when absent - never `"all"`.
pub const SINGLE_VIEW: &[&str] = &[
    "canvas_place",
    "cdp_endpoint",
    "devtools_inspect_at",
    "devtools_open",
    "env_compare_live",
    "recorder_start",
    "view_pop_in",
    "view_pop_out",
    "view_remove",
    "view_set_breakpoint",
];

/// A tool with a guard: a setting that must be on, or keys it refuses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Guard {
    /// The tool.
    pub tool: &'static str,
    /// The setting a person switches on before the tool works, if one.
    pub setting: Option<&'static str>,
    /// What the guard does, in one English sentence.
    pub note: &'static str,
}

/// The guarded tools.
pub const GUARDS: &[Guard] = &[
    Guard {
        tool: "breakpoint_add",
        setting: None,
        note: "Request headers are guarded by workspace trust and refused with code \
               config_key_protected.",
    },
    Guard {
        tool: "breakpoint_update",
        setting: None,
        note: "Request headers are guarded by workspace trust and refused with code \
               config_key_protected.",
    },
    Guard {
        tool: "cdp_endpoint",
        setting: Some("cdp.enabled"),
        note: "Off until a person switches on the setting cdp.enabled (code cdp.disabled) and \
               mcp.allowEvaluate (code evaluate_disabled): the endpoint is full control of the \
               page.",
    },
    Guard {
        tool: "config_set",
        setting: None,
        note: "Keys guarded by workspace trust (headers, network, plugins, extensions) are \
               refused with code config_key_protected.",
    },
    Guard {
        tool: "evaluate",
        setting: Some(ALLOW_EVALUATE),
        note: "Off until a person switches on the setting mcp.allowEvaluate; while off it \
               answers code evaluate_disabled.",
    },
    Guard {
        tool: "extension_install",
        setting: Some("engines.chromium.extensions.allowWebStore"),
        note: "Off while engines.chromium.extensions.allowWebStore is off (code \
               extensions.web_store_off); on, nothing is unpacked before the person confirms \
               the install in rBrowser's window (declined: code extensions.declined).",
    },
    Guard {
        tool: "extension_remove",
        setting: Some("engines.chromium.extensions.allowWebStore"),
        note: "Off while engines.chromium.extensions.allowWebStore is off (code \
               extensions.web_store_off); on, nothing is deleted before the person confirms \
               the removal in rBrowser's window (declined: code extensions.declined).",
    },
    Guard {
        tool: "network_continue",
        setting: Some(ALLOW_EVALUATE),
        note: "Edits (method, URL, headers, body, status) only while mcp.allowEvaluate is on \
               (code evaluate_disabled); a plain continue always.",
    },
    Guard {
        tool: "network_compose",
        setting: Some("network-panel.allowComposeFromMcp"),
        note: "Off until a person switches on the protected setting \
               network-panel.allowComposeFromMcp (code compose_disabled): it sends with the \
               cookies of the person's views.",
    },
    Guard {
        tool: "network_repeat",
        setting: Some("network-panel.allowComposeFromMcp"),
        note: "As network_compose (code compose_disabled).",
    },
    Guard {
        tool: "flow_run",
        setting: None,
        note: "A flow is project code: it runs only in a workspace a person trusted, otherwise \
               it answers code flows.untrusted; no tool grants trust.",
    },
    Guard {
        tool: "recorder_play",
        setting: None,
        note: "As flow_run: the recording plays as a flow, only in a workspace a person \
               trusted, otherwise it answers code flows.untrusted; no tool grants trust.",
    },
    Guard {
        tool: "settings_set",
        setting: None,
        note: "Protected keys (MCP, trust, plugins, the sandbox, paths of programs) are refused \
               with code settings.rejected.protected; only a person changes them.",
    },
    Guard {
        tool: "update_check",
        setting: Some("updates.enabled"),
        note: "Off while updates.enabled is off (problem update.disabled); it reaches the \
               release host and installs nothing.",
    },
    Guard {
        tool: "update_install",
        setting: Some("updates.enabled"),
        note: "Off while updates.enabled is off (code update.disabled); on, it installs \
               nothing: rBrowser shows the offer with its button and only the person's click \
               installs it; the command update.install is refused to MCP.",
    },
];

/// Why `tool` has no command, when [`WITHOUT_COMMAND`] lists it.
#[must_use]
pub fn without_command(tool: &str) -> Option<&'static str> {
    lookup(WITHOUT_COMMAND, tool)
}

/// What `tool` acts on instead of views, when [`UNTARGETED`] lists it.
#[must_use]
pub fn untargeted(tool: &str) -> Option<&'static str> {
    lookup(UNTARGETED, tool)
}

/// The guard of `tool`, when [`GUARDS`] lists it.
#[must_use]
pub fn guard(tool: &str) -> Option<&'static Guard> {
    GUARDS.iter().find(|guard| guard.tool == tool)
}

fn lookup(table: &[(&str, &'static str)], tool: &str) -> Option<&'static str> {
    table
        .iter()
        .find(|(name, _)| *name == tool)
        .map(|(_, text)| *text)
}

/// Every tool the tables name, with the table: what [`super::tools::tool_problems`] holds to
/// the registered tools.
#[must_use]
pub fn named() -> Vec<(&'static str, &'static str)> {
    let mut named: Vec<(&str, &str)> = Vec::new();
    named.extend(
        WITHOUT_COMMAND
            .iter()
            .map(|(tool, _)| ("WITHOUT_COMMAND", *tool)),
    );
    named.extend(UNTARGETED.iter().map(|(tool, _)| ("UNTARGETED", *tool)));
    named.extend(SINGLE_VIEW.iter().map(|tool| ("SINGLE_VIEW", *tool)));
    named.extend(GUARDS.iter().map(|guard| ("GUARDS", guard.tool)));
    named
}
