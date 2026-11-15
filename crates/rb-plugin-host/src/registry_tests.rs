use rb_plugin_types::codes;

use super::Registry;
use crate::manifest::{PluginManifest, parse_manifest};
use crate::test_support::{ManifestBuilder, signing_key};

fn manifest(id: &str, prefix: &str) -> PluginManifest {
    let builder = ManifestBuilder::named(id, prefix).tables(&format!(
        "\n[[tools]]\nname = \"run\"\ndescription = \"plugin.{id}.tools.run\"\n"
    ));
    parse_manifest(builder.text(&signing_key(1)).as_bytes()).expect("valid")
}

#[test]
fn a_second_plugin_with_the_same_prefix_is_refused_with_the_reason() {
    let mut registry = Registry::new();
    registry.admit(&manifest("seo", "seo")).expect("first");
    let refused = registry
        .admit(&manifest("seo-extra", "seo"))
        .expect_err("prefix taken");
    assert_eq!(refused.code(), Some(codes::MCP_PREFIX_TAKEN));
    assert_eq!(refused.param("prefix"), Some("seo"));
    assert_eq!(refused.param("other"), Some("seo"));
    assert_eq!(refused.param("id"), Some("seo-extra"));
    assert!(
        registry.get("seo-extra").is_none(),
        "nothing of it is admitted"
    );
    assert_eq!(registry.tools().count(), 1);
}

#[test]
fn ids_are_unique_and_removing_frees_the_prefix() {
    let mut registry = Registry::new();
    registry.admit(&manifest("seo", "seo")).expect("first");
    let refused = registry
        .admit(&manifest("seo", "seo2"))
        .expect_err("same id");
    assert_eq!(refused.code(), Some(codes::DUPLICATE_ID));
    assert!(registry.remove("seo"));
    assert!(registry.tool("seo_run").is_none());
    registry
        .admit(&manifest("seo-next", "seo"))
        .expect("the prefix is free again");
    assert_eq!(
        registry.tool("seo_run").map(|tool| tool.plugin_id.as_str()),
        Some("seo-next")
    );
}

#[test]
fn a_tool_that_runs_its_callers_script_says_so() {
    let builder = ManifestBuilder::named("shared-console", "sharedconsole").tables(
        "\n[[tools]]\nname = \"exec\"\ndescription = \"plugin.shared-console.tools.exec\"\nevaluates_input = true\n\n[[tools]]\nname = \"list\"\ndescription = \"plugin.shared-console.tools.list\"\n",
    );
    let manifest = parse_manifest(builder.text(&signing_key(1)).as_bytes()).expect("valid");
    let mut registry = Registry::new();
    registry.admit(&manifest).expect("admitted");
    assert!(
        registry
            .tool("sharedconsole_exec")
            .expect("exec")
            .evaluates_input
    );
    assert!(
        !registry
            .tool("sharedconsole_list")
            .expect("list")
            .evaluates_input
    );
}
