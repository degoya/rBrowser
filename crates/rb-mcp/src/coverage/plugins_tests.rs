use super::{PluginTools, plugin_problems, valid_prefix};

fn plugin(id: &str, prefix: Option<&str>, tools: &[&str]) -> PluginTools {
    PluginTools {
        plugin: id.to_owned(),
        prefix: prefix.map(str::to_owned),
        tools: tools.iter().map(|tool| (*tool).to_owned()).collect(),
    }
}

fn core() -> Vec<String> {
    ["breakpoints_apply", "breakpoints_detect", "views_list"]
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

#[test]
fn a_plugin_tool_is_prefix_underscore_name() {
    let analyzer = plugin(
        "breakpoint-analyzer",
        Some("breakpoints"),
        &["detect", "apply"],
    );
    assert_eq!(
        analyzer.mcp_names(),
        vec![
            "breakpoints_detect".to_owned(),
            "breakpoints_apply".to_owned()
        ]
    );
    assert_eq!(plugin_problems(&[analyzer], &core()), Vec::<String>::new());
}

#[test]
fn a_plugin_tool_without_prefix_breaks_the_rule() {
    let bare = plugin("overlays", None, &["overflow_check"]);
    assert!(bare.mcp_names().is_empty());
    assert_eq!(
        plugin_problems(&[bare], &core()),
        vec![
            "overlays: tool `overflow_check` has no mcp_prefix; plugin tools are \
             <mcp_prefix>_<tool>"
                .to_owned()
        ]
    );
}

#[test]
fn a_prefix_two_plugins_hold_is_refused() {
    let first = plugin("overlays", Some("overlay"), &["grid"]);
    let second = plugin("overlay-extra", Some("overlay"), &["ruler"]);
    assert_eq!(
        plugin_problems(&[first, second], &core()),
        vec!["overlay-extra: mcp_prefix `overlay` is taken by overlays".to_owned()]
    );
}

#[test]
fn a_prefix_over_core_tools_the_plugin_does_not_declare_is_refused() {
    let views = plugin("viewer", Some("views"), &["zoom"]);
    assert_eq!(
        plugin_problems(&[views], &core()),
        vec!["viewer: mcp_prefix `views` is an area of the core tools (views_list)".to_owned()]
    );
    let half = plugin("breakpoint-analyzer", Some("breakpoints"), &["detect"]);
    assert_eq!(
        plugin_problems(&[half], &core()),
        vec![
            "breakpoint-analyzer: mcp_prefix `breakpoints` is an area of the core tools \
             (breakpoints_apply)"
                .to_owned()
        ]
    );
}

#[test]
fn malformed_prefixes_and_names_are_refused() {
    assert!(valid_prefix("a11y"));
    assert!(!valid_prefix("color_picker"));
    assert!(!valid_prefix("Color"));
    assert!(!valid_prefix("x"));
    let bad = plugin("picker", Some("color_picker"), &["Pick", "pick", "pick"]);
    let found = plugin_problems(&[bad], &core());
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(found[0].contains("`Pick` is not"), "{found:?}");
    assert!(found[1].contains("`pick` is declared twice"), "{found:?}");
    assert!(found[2].contains("mcp_prefix `color_picker`"), "{found:?}");
}
