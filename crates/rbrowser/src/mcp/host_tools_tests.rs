use rb_engine::ViewHost;
use serde_json::json;

use super::{call, views_list};
use crate::mcp::tests::core;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unknown_host_tool_is_refused() {
    let core = core();
    let failure = call(&core, "format_disk", json!({}))
        .await
        .expect_err("refused");
    assert_eq!(failure.code, "unknown_tool");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bad_arguments_are_refused_before_anything_runs() {
    let core = core();
    let failure = call(&core, "bus_send", json!({ "payload": 1 }))
        .await
        .expect_err("refused");
    assert_eq!(failure.code, "invalid_arguments");
    let failure = call(
        &core,
        "screenshot",
        json!({ "mode": "element", "selector": "" }),
    )
    .await
    .expect_err("refused");
    assert_eq!(failure.code, "invalid_arguments");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sync_set_before_the_hub_started_is_a_failure_not_a_panic() {
    let core = core();
    let failure = call(&core, "sync_set", json!({ "enabled": false }))
        .await
        .expect_err("no hub");
    assert_eq!(failure.code, "command_failed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_visual_tools_reach_the_runs_and_say_why_they_give_nothing() {
    let core = core();
    let failure = call(&core, "visual_compare", json!({ "threshold": 2.0 }))
        .await
        .expect_err("refused");
    assert_eq!(failure.code, "invalid_arguments");
    for tool in ["visual_compare", "visual_baseline_update"] {
        let failure = call(&core, tool, json!({ "target": "all" }))
            .await
            .expect_err("no project");
        assert_eq!(failure.code, "visual.no_project", "{tool}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recorder_export_reaches_the_export_and_refuses_a_view() {
    let core = core();
    let failure = call(&core, "recorder_export", json!({ "target": 1 }))
        .await
        .expect_err("a view");
    assert_eq!(failure.code, "invalid_arguments");
    let failure = call(&core, "recorder_export", json!({ "format": "spec" }))
        .await
        .expect_err("no project");
    assert_eq!(failure.code, "command_failed");
    assert!(
        failure.message.contains("no project"),
        "{}",
        failure.message
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recorder_play_reaches_the_recordings_and_checks_its_arguments() {
    let core = core();
    let failure = call(&core, "recorder_play", json!({ "target": 1 }))
        .await
        .expect_err("not an argument of the host tool");
    assert_eq!(failure.code, "invalid_arguments");
    let failure = call(&core, "recorder_play", json!({ "views": ["watch"] }))
        .await
        .expect_err("no such view");
    assert_eq!(failure.code, "flows.no_views");
    let failure = call(&core, "recorder_play", json!({ "recordingId": "login" }))
        .await
        .expect_err("no project");
    assert_eq!(failure.code, "command_failed");
    assert!(
        failure.message.contains("no project"),
        "{}",
        failure.message
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn views_list_adds_the_engine_state_to_every_view() {
    let core = core();
    let Some(serde_json::Value::Array(views)) = Some(views_list(&core)) else {
        panic!("not a list");
    };
    assert!(!views.is_empty());
    for view in &views {
        assert_eq!(view["engine"], "chromium", "{view}");
        assert_eq!(view["external"], false, "{view}");
        assert_eq!(view["status"], "running", "{view}");
        assert!(view["unsupported"].is_array(), "{view}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn views_list_adds_desync_and_navigation_to_every_view() {
    let core = core();
    let Some(serde_json::Value::Array(views)) = Some(views_list(&core)) else {
        panic!("not a list");
    };
    for view in &views {
        assert!(view["desync"].is_null(), "{view}");
        assert_eq!(view["syncNavigation"], true, "{view}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn views_list_names_the_frameworks_of_every_view() {
    let core = core();
    let first = core.host().views()[0].id;
    let report = rb_sync::FrameworkMessage {
        v: 1,
        top: true,
        frame: None,
        vue: Some(rb_sync::FrameworkInfo {
            version: Some("3.5.13".to_owned()),
            hook: true,
            devtools: true,
        }),
        react: None,
    };
    core.host()
        .framework_hooks()
        .on_report(first, &report)
        .await;
    let Some(serde_json::Value::Array(views)) = Some(views_list(&core)) else {
        panic!("not a list");
    };
    for view in &views {
        if view["id"] == serde_json::to_value(first).expect("id") {
            assert_eq!(
                view["frameworks"],
                json!({ "vue": { "version": "3.5.13", "hook": true, "devtools": true } }),
                "{view}"
            );
        } else {
            assert_eq!(view["frameworks"], json!({}), "{view}");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn views_list_names_the_views_in_a_window_of_their_own() {
    let core = core();
    let first = core.host().views()[0].id;
    core.host()
        .pop_out(first, crate::host::PopOutOptions::default())
        .expect("pops out");
    let Some(serde_json::Value::Array(views)) = Some(views_list(&core)) else {
        panic!("not a list");
    };
    for view in &views {
        let out = view["id"] == serde_json::to_value(first).expect("id");
        assert_eq!(view["detached"], out, "{view}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn views_list_names_the_range_of_every_view() {
    let core = core();
    let Some(serde_json::Value::Array(views)) = Some(views_list(&core)) else {
        panic!("not a list");
    };
    assert!(!views.is_empty());
    let host = core.host();
    for view in &views {
        let id = rb_engine::ViewId(view["id"].as_u64().expect("id") as u32);
        let range = host.footer_range(id).expect("range");
        assert_eq!(view["range"]["from"], range.from, "{view}");
        match range.to {
            Some(to) => assert_eq!(view["range"]["to"], to, "{view}"),
            None => assert!(view["range"]["to"].is_null(), "{view}"),
        }
    }
    // The widest view has no upper end, the narrowest starts at 0.
    assert!(views.iter().any(|view| view["range"]["to"].is_null()));
    assert!(views.iter().any(|view| view["range"]["from"] == 0));
}
