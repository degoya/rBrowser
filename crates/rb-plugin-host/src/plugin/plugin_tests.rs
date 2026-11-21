use std::sync::Arc;

use rb_plugin_types::codes;

use super::{LoadedPlugin, PluginOptions};
use crate::test_support::{KEY_ID, ManifestBuilder, ScriptedServices, signing_key};
use crate::testbed::Bed;
use crate::{PluginHealth, PluginVerifier, VerifiedPackage};

fn verified(builder: &ManifestBuilder, bed: &Bed) -> VerifiedPackage {
    let key = signing_key(1);
    let package = builder.package(&bed.build(), &key, true, &[]);
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), key.verifying_key())
        .expect("trust");
    verifier.verify_bytes(&package).expect("verifies")
}

fn with_command(builder: ManifestBuilder) -> ManifestBuilder {
    builder.tables(
        "\n[[tools]]\nname = \"run\"\ndescription = \"plugin.sample.tools.run\"\n\
         \n[[commands]]\nid = \"run\"\ntitle = \"plugin.sample.commands.run\"\ntool = \"run\"\n",
    )
}

#[tokio::test]
async fn a_signed_feature_plugin_loads_and_answers() {
    let builder = with_command(ManifestBuilder::default());
    let plugin = LoadedPlugin::load(
        &verified(&builder, &Bed::feature()),
        PluginOptions::default(),
    )
    .expect("loads");
    plugin.activate().await.expect("activates");
    assert_eq!(plugin.run_command("run", "{}").await.expect("runs"), "{}");
    assert_eq!(plugin.call_tool("run", "{}").await.expect("calls"), "{}");
    let unknown = plugin
        .run_command("nope", "{}")
        .await
        .expect_err("undeclared");
    assert_eq!(unknown.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[tokio::test]
async fn an_endless_plugin_is_stopped_by_its_limit_and_the_host_stays_healthy() {
    // The most fuel a manifest may ask for, so the 1 s epoch deadline is what ends the call.
    let builder = with_command(
        ManifestBuilder::default()
            .limit("max_concurrent = 2")
            .limit(&format!("fuel = {}", crate::MAX_FUEL)),
    );
    let health = Arc::new(PluginHealth::new(3));
    let options = PluginOptions {
        health: Arc::clone(&health),
        ..PluginOptions::default()
    };
    let endless = LoadedPlugin::load(
        &verified(&builder, &Bed::feature().endless("run-command")),
        options,
    )
    .expect("loads");
    let stopped = endless
        .run_command("run", "{}")
        .await
        .expect_err("never ends");
    assert_eq!(stopped.code(), Some(codes::TIMED_OUT));

    // The same plugin's other exports still answer: a trap ends one call, not the plugin.
    endless
        .activate()
        .await
        .expect("the next call starts afresh");
    // Another plugin runs as before.
    let healthy = LoadedPlugin::load(
        &verified(&builder, &Bed::feature()),
        PluginOptions::default(),
    )
    .expect("loads");
    assert_eq!(healthy.run_command("run", "{}").await.expect("runs"), "{}");
    assert!(!health.is_suspended(), "one failure does not suspend");
}

#[tokio::test]
async fn a_plugin_out_of_fuel_is_stopped_too() {
    let builder = with_command(ManifestBuilder::default().limit("fuel = 100000"));
    let plugin = LoadedPlugin::load(
        &verified(&builder, &Bed::feature().endless("run-command")),
        PluginOptions::default(),
    )
    .expect("loads");
    let stopped = plugin.run_command("run", "{}").await.expect_err("no fuel");
    assert_eq!(stopped.code(), Some(codes::OUT_OF_FUEL));
}

#[tokio::test]
async fn a_plugin_that_keeps_crashing_is_suspended() {
    let builder = with_command(ManifestBuilder::default());
    let health = Arc::new(PluginHealth::new(2));
    let plugin = LoadedPlugin::load(
        &verified(&builder, &Bed::feature().crashing("run-command")),
        PluginOptions {
            health: Arc::clone(&health),
            ..PluginOptions::default()
        },
    )
    .expect("loads");
    for _ in 0..2 {
        let crashed = plugin.run_command("run", "{}").await.expect_err("traps");
        assert_eq!(crashed.code(), Some(codes::CRASHED));
    }
    let suspended = plugin.activate().await.expect_err("suspended");
    assert_eq!(suspended.code(), Some(codes::SUSPENDED));
    health.resume();
    plugin.activate().await.expect("resumed");
}

#[tokio::test]
async fn a_write_outside_the_out_folder_is_refused_inside_the_sandbox() {
    let project = tempfile::tempdir().expect("tempdir");
    let builder = with_command(ManifestBuilder::default().capability("out-write = true"));
    let options = PluginOptions {
        project_root: Some(project.path().to_path_buf()),
        services: Arc::new(ScriptedServices::default()),
        ..PluginOptions::default()
    };
    let escaping = LoadedPlugin::load(
        &verified(
            &builder,
            &Bed::feature().writing("run-command", "../escape.txt"),
        ),
        options.clone(),
    )
    .expect("loads");
    assert_eq!(
        escaping.run_command("run", "{}").await.expect("answers"),
        "\"denied\""
    );
    assert!(!project.path().join(".rbrowser/out/escape.txt").exists());

    let inside = LoadedPlugin::load(
        &verified(
            &builder,
            &Bed::feature().writing("run-command", "report.txt"),
        ),
        options,
    )
    .expect("loads");
    assert_eq!(
        inside.run_command("run", "{}").await.expect("answers"),
        "\"written\""
    );
    assert_eq!(
        std::fs::read(project.path().join(".rbrowser/out/sample/report.txt")).expect("written"),
        b"x"
    );
}

#[tokio::test]
async fn every_kind_answers_through_its_world() {
    let check = LoadedPlugin::load(
        &verified(
            &ManifestBuilder::default().kinds(&["check-plugin"]),
            &Bed::check(),
        ),
        PluginOptions::default(),
    )
    .expect("loads");
    let rules = check.describe().await.expect("rules");
    assert_eq!(rules[0].id, "basic");
    assert_eq!(rules[0].severity, rb_plugin_types::Severity::Warning);
    assert_eq!(rules[0].shared_id.as_deref(), Some("shared-basic"));
    assert_eq!(rules[0].help_url, None);
    let ctx = rb_plugin_types::CheckContext {
        view: Some(1),
        url: Some("https://site.test/".to_owned()),
        rules: vec!["basic".to_owned()],
        args: "{}".to_owned(),
    };
    assert!(check.run_check(&ctx).await.expect("runs").is_empty());
    let world = check.activate().await.expect_err("no feature");
    assert_eq!(world.code(), Some(codes::WORLD_MISSING));

    let analyzer = LoadedPlugin::load(
        &verified(
            &ManifestBuilder::default().kinds(&["analyzer-plugin"]),
            &Bed::analyzer(),
        ),
        PluginOptions::default(),
    )
    .expect("loads");
    assert!(analyzer.analyze("{}").await.expect("analyzes").is_empty());

    let exporter = LoadedPlugin::load(
        &verified(
            &ManifestBuilder::default().kinds(&["export-plugin"]),
            &Bed::exporter(),
        ),
        PluginOptions::default(),
    )
    .expect("loads");
    let formats = exporter.formats().await.expect("formats");
    assert_eq!(formats[0].extension, "txt");
    let output = exporter.render("text", "{}").await.expect("renders");
    assert_eq!(output.file_name, "out.txt");
}

#[tokio::test]
async fn a_check_answers_its_findings_through_the_world() {
    let check = LoadedPlugin::load(
        &verified(
            &ManifestBuilder::default().kinds(&["check-plugin"]),
            &Bed::check_finding_two(),
        ),
        PluginOptions::default(),
    )
    .expect("loads");
    let found = check
        .run_check(&rb_plugin_types::CheckContext {
            rules: vec!["basic".to_owned()],
            args: "{}".to_owned(),
            ..rb_plugin_types::CheckContext::default()
        })
        .await
        .expect("runs");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].rule, "basic");
    assert_eq!(found[0].message, "plugin.sample.findings.basic");
    assert_eq!(found[0].url.as_deref(), Some("https://site.test/"));
    assert_eq!(found[1].selector.as_deref(), Some("img.b"));
    assert_eq!(
        found[1].path.as_deref(),
        Some("html > body > img:nth-of-type(2)")
    );
    assert_eq!((found[0].view, found[0].data.as_deref()), (None, None));
}

fn with_page_script(builder: ManifestBuilder) -> ManifestBuilder {
    builder.tables("\n[[page_scripts]]\npath = \"page/a.js\"\n")
}

fn verified_with(
    builder: &ManifestBuilder,
    bed: &Bed,
    extra: &[(String, Vec<u8>)],
) -> VerifiedPackage {
    let key = signing_key(1);
    let package = builder.package(&bed.build(), &key, true, extra);
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), key.verifying_key())
        .expect("trust");
    verifier.verify_bytes(&package).expect("verifies")
}

#[tokio::test]
async fn a_page_message_reaches_the_component_as_the_page_tool() {
    let builder = with_page_script(ManifestBuilder::default());
    let script = vec![("page/a.js".to_owned(), b"rbPlugin.send(1);".to_vec())];
    let plugin = LoadedPlugin::load(
        &verified_with(&builder, &Bed::feature().echoing("call-tool"), &script),
        PluginOptions::default(),
    )
    .expect("loads");
    let answer = plugin
        .page_message(4, &serde_json::json!({ "hello": "world" }))
        .await
        .expect("answers");
    let answer: serde_json::Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(
        answer,
        serde_json::json!({ "view": 4, "message": { "hello": "world" } })
    );
}

#[tokio::test]
async fn a_plugin_without_page_scripts_takes_no_page_message() {
    let plugin = LoadedPlugin::load(
        &verified(&ManifestBuilder::default(), &Bed::feature()),
        PluginOptions::default(),
    )
    .expect("loads");
    let refused = plugin
        .page_message(1, &serde_json::json!(null))
        .await
        .expect_err("no page script");
    assert_eq!(refused.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

#[tokio::test]
async fn the_rules_request_reaches_a_plugin_granted_network_rules_as_the_rules_tool() {
    let builder = ManifestBuilder::default().capability("network-rules = true");
    let plugin = LoadedPlugin::load(
        &verified(&builder, &Bed::feature().echoing("call-tool")),
        PluginOptions::default(),
    )
    .expect("loads");
    let answer = plugin
        .network_rules(r#"{"$network":{"trusted":true}}"#)
        .await
        .expect("answers");
    let answer: serde_json::Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(
        answer,
        serde_json::json!({ "$network": { "trusted": true } })
    );
    assert_eq!(crate::NETWORK_RULES_TOOL, "@rules");
}

#[tokio::test]
async fn a_plugin_not_granted_network_rules_takes_no_rules_request() {
    let plugin = LoadedPlugin::load(
        &verified(&ManifestBuilder::default(), &Bed::feature()),
        PluginOptions::default(),
    )
    .expect("loads");
    let refused = plugin.network_rules("{}").await.expect_err("not granted");
    assert_eq!(refused.code(), Some(codes::UNKNOWN_CONTRIBUTION));
}

/// RB-090-32: every call is counted - its wait for a slot (`plugin.wait`) and its run
/// (`plugin.call`).
#[tokio::test]
async fn a_call_is_counted_with_its_wait_and_its_run() {
    use rb_util::perf::{global, names};
    let count = |name: &str| {
        global()
            .snapshot()
            .iter()
            .find(|series| series.name == name)
            .map_or(0, |series| series.count)
    };
    let builder = with_command(ManifestBuilder::default());
    let plugin = LoadedPlugin::load(
        &verified(&builder, &Bed::feature()),
        PluginOptions::default(),
    )
    .expect("loads");
    plugin.activate().await.expect("activates");
    let (waits, calls) = (count(names::PLUGIN_WAIT), count(names::PLUGIN_CALL));
    plugin.call_tool("run", "{}").await.expect("calls");
    assert!(count(names::PLUGIN_WAIT) > waits);
    assert!(count(names::PLUGIN_CALL) > calls);
}
