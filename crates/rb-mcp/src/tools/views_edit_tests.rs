use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{AddParams, AddSizeParams, RemoveParams};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

#[tokio::test]
async fn view_add_runs_the_command_with_the_breakpoint_and_the_canvas_point() {
    let fake = Arc::new(FakeBackend::default());
    let params = AddParams {
        breakpoint: "mobile".to_owned(),
        x: Some(10.0),
        y: Some(20.0),
    };
    let (error, _) = answer_json(
        &server(&fake)
            .view_add(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    let (id, args) = fake.calls().pop().expect("a command");
    assert_eq!(id, "view.add");
    assert_eq!(
        args,
        json!({ "breakpoint": "mobile", "x": 10.0, "y": 20.0 })
    );
    let all = AddParams {
        breakpoint: "all".to_owned(),
        x: None,
        y: None,
    };
    server(&fake)
        .view_add(Parameters(all))
        .await
        .expect("answer");
    let (_, args) = fake.calls().pop().expect("a command");
    assert_eq!(args, json!({ "breakpoint": "all" }));
}

#[tokio::test]
async fn view_remove_passes_a_breakpoint_or_a_view_id_through() {
    let fake = Arc::new(FakeBackend::default());
    let params = RemoveParams {
        target: Some(TargetArg::Name("desktop".to_owned())),
    };
    let (error, _) = answer_json(
        &server(&fake)
            .view_remove(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    let (id, args) = fake.calls().pop().expect("a command");
    assert_eq!(id, "view.remove");
    // A breakpoint id goes through as such: one hidden already answers changed: false.
    assert_eq!(args, json!({ "breakpoint": "desktop" }));
    server(&fake)
        .view_remove(Parameters(RemoveParams {
            target: Some(TargetArg::Id(2)),
        }))
        .await
        .expect("answer");
    let (_, args) = fake.calls().pop().expect("a command");
    assert_eq!(args, json!({ "view": 2 }));
    server(&fake)
        .view_remove(Parameters(RemoveParams::default()))
        .await
        .expect("answer");
    let (_, args) = fake.calls().pop().expect("a command");
    assert_eq!(args, json!({}), "the active view is the command's default");
    let (error, answer) = answer_json(
        &server(&fake)
            .view_remove(Parameters(RemoveParams {
                target: Some(TargetArg::all()),
            }))
            .await
            .expect("answer"),
    );
    assert!(error, "{answer}");
}

#[tokio::test]
async fn view_add_size_passes_the_size_and_the_name() {
    let fake = Arc::new(FakeBackend::default());
    let params = AddSizeParams {
        width: Some(1440),
        height: Some(900),
        device: None,
        name: Some("wide".to_owned()),
        dpr: None,
        x: None,
        y: None,
    };
    let (error, _) = answer_json(
        &server(&fake)
            .view_add_size(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    let (id, args) = fake.calls().pop().expect("a command");
    assert_eq!(id, "view.add_size");
    assert_eq!(
        args,
        json!({ "width": 1440, "height": 900, "name": "wide" })
    );
}

#[tokio::test]
async fn view_add_size_takes_a_device_preset_instead_of_a_size() {
    let fake = Arc::new(FakeBackend::default());
    let params: AddSizeParams =
        serde_json::from_value(json!({ "device": "iphone-15" })).expect("a device alone");
    let (error, _) = answer_json(
        &server(&fake)
            .view_add_size(Parameters(params))
            .await
            .expect("answer"),
    );
    assert!(!error);
    let (id, args) = fake.calls().pop().expect("a command");
    assert_eq!(id, "view.add_size");
    assert_eq!(args, json!({ "device": "iphone-15" }));
}

#[tokio::test]
async fn view_add_size_without_a_size_or_a_device_runs_nothing() {
    // Without arguments the command opens the picker in the window - never over MCP.
    for params in [
        json!({}),
        json!({ "name": "wide" }),
        json!({ "width": 800 }),
    ] {
        let fake = Arc::new(FakeBackend::default());
        let params: AddSizeParams = serde_json::from_value(params).expect("parses");
        let (error, answer) = answer_json(
            &server(&fake)
                .view_add_size(Parameters(params))
                .await
                .expect("answer"),
        );
        assert!(error, "{answer}");
        assert!(fake.calls().is_empty(), "{:?}", fake.calls());
    }
}
