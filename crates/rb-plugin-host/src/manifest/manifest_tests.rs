use rb_plugin_types::{Capability, PluginKind, codes};

use super::{RESERVED_IDS, parse_manifest};
use crate::refusal::refusal_code;
use crate::test_support::{ManifestBuilder, signing_key};

fn text(builder: &ManifestBuilder) -> String {
    builder.text(&signing_key(1))
}

#[test]
fn the_default_test_manifest_parses() {
    let manifest = parse_manifest(text(&ManifestBuilder::default()).as_bytes()).expect("valid");
    assert_eq!(manifest.id, "sample");
    assert_eq!(manifest.kinds, vec![PluginKind::Feature]);
    assert_eq!(manifest.mcp_prefix.as_deref(), Some("sample"));
    assert_eq!(manifest.command_id("run"), "plugin.sample.run");
    assert_eq!(manifest.tool_name("run").as_deref(), Some("sample_run"));
}

#[test]
fn a_readable_id_is_required_and_reverse_dns_is_refused() {
    for id in ["breakpoint-analyzer", "seo", "a11y-axe"] {
        let builder = ManifestBuilder::named(id, "x1");
        assert!(parse_manifest(text(&builder).as_bytes()).is_ok(), "{id}");
    }
    for id in [
        "net.degoya.seo",
        "019d0000-0000-7000-8000-0000000000a1x",
        "Seo",
        "-seo",
        "seo-",
        "se--o",
        "s",
        "plugins",
        "checks",
    ] {
        let builder = ManifestBuilder::named(id, "x1");
        assert!(parse_manifest(text(&builder).as_bytes()).is_err(), "{id}");
    }
}

/// The core's own folders under `.rbrowser/out/` are no plugin's `out-write` area: a plugin
/// named like one would write into the core's output without trust (security review RB-110-12).
#[test]
fn the_cores_out_folders_are_reserved_ids() {
    for id in [
        "checks",
        "engine-compare",
        "env-compare",
        "exports",
        "flows",
        "recordings",
        "reports",
        "screenshots",
        "video",
    ] {
        assert!(RESERVED_IDS.contains(&id), "{id}");
        let builder = ManifestBuilder::named(id, "x1");
        assert!(parse_manifest(text(&builder).as_bytes()).is_err(), "{id}");
    }
}

#[test]
fn the_mcp_prefix_is_one_word_and_no_core_area() {
    let builder = ManifestBuilder::named("sample", "seo_tools");
    assert!(parse_manifest(text(&builder).as_bytes()).is_err());
    let builder = ManifestBuilder::named("sample", "settings");
    let error = parse_manifest(text(&builder).as_bytes()).expect_err("reserved");
    assert_eq!(refusal_code(&error), codes::MCP_PREFIX_RESERVED);
    let tools = ManifestBuilder {
        mcp_prefix: None,
        ..ManifestBuilder::default()
    }
    .tables("\n[[tools]]\nname = \"run\"\ndescription = \"plugin.sample.tools.run\"\n");
    assert!(
        parse_manifest(text(&tools).as_bytes()).is_err(),
        "tools need a prefix"
    );
}

#[test]
fn capabilities_parse_with_their_settings_and_unknown_ones_refuse() {
    let builder = ManifestBuilder::default()
        .capability("http = { rate_limit_per_second = 2 }")
        .capability("cdp = { domains = [\"CSS\", \"DOM\"] }")
        .capability("project-read = { globs = [\"src/**/*.css\"] }")
        .capability("out-write = true")
        .capability("findings = false");
    let manifest = parse_manifest(text(&builder).as_bytes()).expect("valid");
    assert!(manifest.grants(Capability::Http));
    assert!(manifest.grants(Capability::OutWrite));
    assert!(!manifest.grants(Capability::Findings));
    assert_eq!(manifest.grants.http_rate_per_second, 2);
    assert_eq!(manifest.grants.cdp_domains, ["CSS", "DOM"]);
    assert!(manifest.grants.glob_matches("src/a/b.css"));
    assert!(!manifest.grants.glob_matches("src/a/b.js"));

    for line in [
        "filesystem = true",
        "cdp = true",
        "cdp = { domains = [\"Target\"] }",
        "http = { rate_limit_per_second = 0 }",
        "project-read = { globs = [\"../**\"] }",
        "views = { extra = 1 }",
    ] {
        let builder = ManifestBuilder::default().capability(line);
        assert!(parse_manifest(text(&builder).as_bytes()).is_err(), "{line}");
    }
}

#[test]
fn contributions_are_checked() {
    let good = ManifestBuilder::default().tables(
        "\n[[tools]]\nname = \"run\"\ndescription = \"plugin.sample.tools.run\"\n\
         \n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\ntool = \"run\"\n\
         \n[[cli]]\nname = \"run\"\ncommand = \"run\"\n",
    );
    let manifest = parse_manifest(text(&good).as_bytes()).expect("valid");
    assert_eq!(manifest.commands.len(), 1);

    for tables in [
        // A title outside the plugin's keys.
        "\n[[commands]]\nid = \"run\"\ntitle = \"commands.seo.check\"\nmcp_omitted = \"ui-only\"\n",
        // Neither a tool nor a reason.
        "\n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\n",
        // A tool that is not declared.
        "\n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\ntool = \"nope\"\n",
        // A group that does not exist.
        "\n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\nmcp_omitted = \"x\"\ngroup = \"misc\"\n",
        // A page script outside page/.
        "\n[[page_scripts]]\npath = \"overlay.js\"\n",
    ] {
        let builder = ManifestBuilder::default().tables(tables);
        assert!(
            parse_manifest(text(&builder).as_bytes()).is_err(),
            "{tables}"
        );
    }
}

#[test]
fn only_a_feature_plugin_contributes_commands() {
    let builder = ManifestBuilder::default().kinds(&["check-plugin"]).tables(
        "\n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\nmcp_omitted = \"ui-only\"\n",
    );
    assert!(parse_manifest(text(&builder).as_bytes()).is_err());
}

#[test]
fn versions_and_unknown_fields_are_refused() {
    let base = text(&ManifestBuilder::default());
    let other_api = base.replace("api_version = \"0.1.0\"", "api_version = \"9.0.0\"");
    let error = parse_manifest(other_api.as_bytes()).expect_err("unknown contract");
    assert_eq!(refusal_code(&error), codes::API_VERSION);
    let not_semver = base.replace("version = \"1.0.0\"", "version = \"one\"");
    assert!(parse_manifest(not_semver.as_bytes()).is_err());
    let unknown = base.replace("name = \"Sample\"", "name = \"Sample\"\ncolour = \"red\"");
    assert!(parse_manifest(unknown.as_bytes()).is_err());
    let no_english = base.replace("en = \"locales/en.json\"", "de = \"locales/de.json\"");
    assert!(parse_manifest(no_english.as_bytes()).is_err());
}

#[test]
fn a_panel_is_a_side_bar_tab_unless_it_says_dialog() {
    let builder = ManifestBuilder::default().tables(
        "\n[[panels]]\nid = \"main\"\ntitle = \"plugin.sample.panels.main\"\nentry = \"panel/index.html\"\n\
         \n[[panels]]\nid = \"ask\"\ntitle = \"plugin.sample.panels.ask\"\nentry = \"panel/ask.html\"\nplacement = \"dialog\"\n",
    );
    let manifest = parse_manifest(text(&builder).as_bytes()).expect("valid");
    assert_eq!(manifest.panels[0].placement, super::PanelPlacement::Sidebar);
    assert_eq!(manifest.panels[1].placement, super::PanelPlacement::Dialog);
    let unknown = ManifestBuilder::default().tables(
        "\n[[panels]]\nid = \"main\"\ntitle = \"plugin.sample.panels.main\"\nentry = \"panel/index.html\"\nplacement = \"floating\"\n",
    );
    assert!(parse_manifest(text(&unknown).as_bytes()).is_err());
}

/// `text` with `line` placed among the manifest's own keys, before `[locales]`.
fn with_top_level(builder: &ManifestBuilder, line: &str) -> String {
    text(builder).replacen("[locales]", &format!("{line}\n\n[locales]"), 1)
}

#[test]
fn the_context_menu_list_names_declared_commands_once() {
    let builder = ManifestBuilder::default().tables(
        "\n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\nmcp_omitted = \"ui-only\"\n\
         \n[[commands]]\nid = \"clear\"\ntitle = \"plugin.sample.commands.clear\"\nmcp_omitted = \"ui-only\"\n",
    );
    let none = parse_manifest(text(&builder).as_bytes()).expect("valid");
    assert!(none.context_menu.is_empty(), "the list is optional");
    let listed =
        parse_manifest(with_top_level(&builder, "context_menu = [\"run\", \"clear\"]").as_bytes())
            .expect("valid");
    assert_eq!(
        listed.context_menu,
        vec!["run".to_owned(), "clear".to_owned()]
    );
    for line in [
        // A command the manifest does not declare.
        "context_menu = [\"nope\"]",
        // Twice.
        "context_menu = [\"run\", \"run\"]",
    ] {
        let error = parse_manifest(with_top_level(&builder, line).as_bytes())
            .expect_err(line)
            .to_string();
        assert!(error.starts_with("context_menu"), "{line}: {error}");
    }
}

/// A manifest with one panel carrying `icon_line` (empty: none).
fn panel_with_icon(icon_line: &str) -> String {
    text(&ManifestBuilder::default().tables(&format!(
        "\n[[panels]]\nid = \"main\"\ntitle = \"plugin.sample.panels.main\"\nentry = \"panel/index.html\"\n{icon_line}\n"
    )))
}

#[test]
fn a_panel_icon_is_an_optional_lucide_name_never_a_url_or_markup() {
    let none = parse_manifest(panel_with_icon("").as_bytes()).expect("valid");
    assert_eq!(none.panels[0].icon, None);
    for name in ["type", "network", "text-cursor-input", "chart-pie", "a1"] {
        let manifest =
            parse_manifest(panel_with_icon(&format!("icon = \"{name}\"")).as_bytes()).expect(name);
        assert_eq!(manifest.panels[0].icon.as_deref(), Some(name));
    }
    let long = "a".repeat(41);
    for bad in [
        "",
        "Type",
        "https://example.com/x.svg",
        "data:image/svg+xml,x",
        "<svg>",
        "lucide:type",
        "a b",
        "../x",
        long.as_str(),
    ] {
        let error = parse_manifest(panel_with_icon(&format!("icon = {bad:?}")).as_bytes())
            .expect_err(bad)
            .to_string();
        assert!(error.starts_with("panels.main.icon"), "{bad}: {error}");
    }
}
