use super::{INSTRUCTIONS, RbMcpServer};

/// The core tools, as the job files and the commands name them (RB-030-01 and the tools of
/// sync, input, scroll, bus, screenshot, checks and sessions it registers).
const CORE_TOOLS: &[&str] = &[
    "android_devices",
    "animations_set",
    "app_info",
    "app_perf",
    "back",
    "bookmarks_list",
    "bookmarks_make_relative",
    "breakpoint_add",
    "breakpoint_remove",
    "breakpoint_update",
    "breakpoints_apply",
    "breakpoints_detect",
    "bug_report",
    "bus_send",
    "canvas_place",
    "canvas_zoom",
    "cdp_endpoint",
    "checks_cancel",
    "checks_export",
    "checks_run",
    "click",
    "clock_set",
    "compare_engines",
    "config_get",
    "config_set",
    "console_logs",
    "cookies_copy",
    "devtools_inspect_at",
    "devtools_open",
    "diagnostics",
    "emulation_set",
    "engine_restart",
    "engines_list",
    "env_compare",
    "env_compare_cancel",
    "env_compare_live",
    "env_compare_result",
    "environment_set",
    "errors",
    "evaluate",
    "extension_install",
    "extension_list",
    "extension_remove",
    "flow_list",
    "flow_run",
    "forward",
    "hover",
    "ios_devices",
    "layout_save_preset",
    "layout_set",
    "navigate",
    "network_abort",
    "network_breakpoints",
    "network_compose",
    "network_continue",
    "network_repeat",
    "network_requests",
    "panels_dock",
    "plugins_catalog_list",
    "plugins_list",
    "portal_devices",
    "portal_disconnect",
    "portal_present_status",
    "portal_present_stop",
    "portal_stop",
    "press_key",
    "project_info",
    "project_open",
    "pseudo_locale",
    "record_video",
    "recorder_export",
    "recorder_play",
    "recorder_start",
    "recorder_step_add",
    "recorder_step_remove",
    "recorder_step_toggle",
    "recorder_step_try",
    "recorder_step_update",
    "recorder_stop",
    "reload",
    "screenshot",
    "script_run",
    "scroll",
    "session_assign",
    "session_clear",
    "settings_get",
    "settings_set",
    "sitemap_cancel",
    "sitemap_capture",
    "sitemap_gallery",
    "sitemap_status",
    "snapshot",
    "status_messages",
    "sync_set",
    "type",
    "update_check",
    "update_install",
    "update_status",
    "video_list",
    "view_add",
    "view_add_android",
    "view_add_size",
    "view_metrics",
    "view_page_zoom",
    "view_pop_in",
    "view_pop_out",
    "view_remove",
    "view_set_breakpoint",
    "views_force_state",
    "views_list",
    "views_restore_last",
    "views_set_engine",
    "visual_baseline_update",
    "visual_compare",
    "wsl_info",
];

#[test]
fn every_core_tool_is_registered_and_nothing_else() {
    assert_eq!(RbMcpServer::tool_names(), CORE_TOOLS);
}

#[test]
fn no_tool_can_touch_trust_or_install_plugins() {
    for name in RbMcpServer::tool_names() {
        assert!(!name.contains("trust"), "{name}");
        // `extension_install` (RB-080-23) unpacks nothing before the person confirms it in
        // rBrowser's window, `update_install` (RB-090-14) installs nothing before the person's
        // click; no tool installs a plugin.
        assert!(
            !name.contains("install") || name == "extension_install" || name == "update_install",
            "{name}"
        );
    }
}

#[test]
fn the_instructions_name_only_real_tools() {
    let tools = RbMcpServer::tool_names();
    let named: Vec<&str> = INSTRUCTIONS
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|word| {
            word.chars().all(|c| c.is_ascii_lowercase() || c == '_') && word.contains('_')
                || tools.iter().any(|tool| tool == word)
        })
        .collect();
    assert!(named.len() > 10, "{named:?}");
    for word in named {
        assert!(tools.iter().any(|tool| tool == word), "`{word}` is no tool");
    }
}

#[test]
fn every_tool_has_a_description_and_an_object_schema() {
    for tool in RbMcpServer::router().list_all() {
        assert!(
            tool.description
                .as_deref()
                .is_some_and(|text| text.len() > 20),
            "{}",
            tool.name
        );
        assert_eq!(
            tool.input_schema.get("type"),
            Some(&serde_json::json!("object")),
            "{}",
            tool.name
        );
    }
}
