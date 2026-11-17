use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Map, json};

use super::{
    ApplyParams, CanvasPlaceParams, CanvasZoomLevel, CanvasZoomParams, DetectParams,
    EmulationParams, EnvironmentParams, LayoutParams, SavePresetParams, SetSizeParams,
};
use crate::target::TargetArg;
use crate::testing::{FakeBackend, answer_json, server};

#[tokio::test]
async fn detect_always_passes_arguments_so_no_wizard_opens() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .breakpoints_detect(Parameters(DetectParams::default()))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![("breakpoints.analyze".to_owned(), json!({ "urls": [] }))]
    );
}

#[tokio::test]
async fn apply_passes_its_choice() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .breakpoints_apply(Parameters(ApplyParams {
            proposal_id: Some(3),
            selected: vec!["md".to_owned()],
            replace: false,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls()[0],
        (
            "breakpoints.append_detected".to_owned(),
            json!({ "proposalId": 3, "selected": ["md"], "replace": false })
        )
    );
}

#[tokio::test]
async fn view_set_breakpoint_targets_one_view() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .view_set_breakpoint(Parameters(SetSizeParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
            breakpoint: Some("mobile".to_owned()),
            ..SetSizeParams::default()
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls()[0],
        (
            "view.set_size".to_owned(),
            json!({ "view": 2, "breakpoint": "mobile" })
        )
    );
    let (error, answer) = answer_json(
        &server
            .view_set_breakpoint(Parameters(SetSizeParams {
                target: Some(TargetArg::all()),
                width: Some(500),
                ..SetSizeParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(answer["code"], "target_not_single");
}

/// A free size for one view: width, height and dpr reach `view.set_size` (RB-010-16).
#[tokio::test]
async fn view_set_breakpoint_passes_a_free_size() {
    let fake = Arc::new(FakeBackend::default());
    let (error, _) = answer_json(
        &server(&fake)
            .view_set_breakpoint(Parameters(SetSizeParams {
                target: Some(TargetArg::Name("mobile".to_owned())),
                width: Some(500),
                height: Some(900),
                dpr: Some(2.0),
                ..SetSizeParams::default()
            }))
            .await
            .expect("answer"),
    );
    assert!(!error);
    assert_eq!(
        fake.calls()[0],
        (
            "view.set_size".to_owned(),
            json!({ "view": 1, "width": 500, "height": 900, "dpr": 2.0 })
        )
    );
}

#[tokio::test]
async fn emulation_set_resolves_a_breakpoint_to_its_views() {
    let fake = Arc::new(FakeBackend::default());
    let mut fields = Map::new();
    fields.insert("colorScheme".to_owned(), json!("dark"));
    server(&fake)
        .emulation_set(Parameters(EmulationParams {
            target: Some(TargetArg::Name("mobile".to_owned())),
            fields,
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls()[0],
        (
            "emulation.set".to_owned(),
            json!({ "target": 1, "fields": { "colorScheme": "dark" } })
        )
    );
}

#[tokio::test]
async fn layout_and_environment_run_their_commands() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .layout_set(Parameters(LayoutParams {
            preset: Some("phones".to_owned()),
            focus_mode: Some(true),
            focus: Some("mobile".to_owned()),
            ..LayoutParams::default()
        }))
        .await
        .expect("answer");
    server
        .environment_set(Parameters(EnvironmentParams {
            id: "staging".to_owned(),
        }))
        .await
        .expect("answer");
    let (error, _) = answer_json(
        &server
            .layout_set(Parameters(LayoutParams::default()))
            .await
            .expect("answer"),
    );
    assert!(error);
    assert_eq!(
        fake.calls(),
        vec![
            ("layout.select_preset".to_owned(), json!({ "id": "phones" })),
            (
                "layout.focus_mode".to_owned(),
                json!({ "enabled": true, "focus": "mobile" })
            ),
            ("environments.select".to_owned(), json!({ "id": "staging" })),
        ]
    );
}

#[tokio::test]
async fn layout_set_switches_the_mode_with_its_fields() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    server
        .layout_set(Parameters(LayoutParams {
            mode: Some("split".to_owned()),
            ratio: Some(0.6),
            primary: Some(vec!["desktop".to_owned()]),
            ..LayoutParams::default()
        }))
        .await
        .expect("answer");
    server
        .layout_set(Parameters(LayoutParams {
            mode: Some("canvas".to_owned()),
            ..LayoutParams::default()
        }))
        .await
        .expect("answer");
    for refused in [
        LayoutParams {
            mode: Some("diagonal".to_owned()),
            ..LayoutParams::default()
        },
        LayoutParams {
            snap: Some(8),
            ..LayoutParams::default()
        },
    ] {
        let (error, _) = answer_json(
            &server
                .layout_set(Parameters(refused))
                .await
                .expect("answer"),
        );
        assert!(error);
    }
    assert_eq!(
        fake.calls(),
        vec![
            (
                "layout.mode.split".to_owned(),
                json!({ "ratio": 0.6, "primary": ["desktop"] })
            ),
            ("layout.mode.canvas".to_owned(), json!(null)),
        ]
    );
}

#[tokio::test]
async fn canvas_place_passes_the_view_and_its_place() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .canvas_place(Parameters(CanvasPlaceParams {
            target: Some(TargetArg::Name("desktop".to_owned())),
            x: 16.0,
            y: 32.0,
            zoom: Some(0.5),
        }))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        vec![(
            "layout.canvas.place".to_owned(),
            json!({ "view": 2, "x": 16.0, "y": 32.0, "zoom": 0.5 })
        )]
    );
}

#[tokio::test]
async fn save_preset_passes_the_name_and_the_file() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    for file in [None, Some("project")] {
        server
            .layout_save_preset(Parameters(SavePresetParams {
                id: "board 2".to_owned(),
                file: file.map(str::to_owned),
            }))
            .await
            .expect("answer");
    }
    assert_eq!(
        fake.calls(),
        vec![
            ("layout.save_preset".to_owned(), json!({ "id": "board 2" })),
            (
                "layout.save_preset".to_owned(),
                json!({ "id": "board 2", "file": "project" })
            ),
        ]
    );
}

#[tokio::test]
async fn canvas_zoom_sets_a_level_or_fits() {
    let fake = Arc::new(FakeBackend::default());
    let server = server(&fake);
    for level in [
        None,
        Some(CanvasZoomLevel::Level(1.5)),
        Some(CanvasZoomLevel::Named("fit".to_owned())),
    ] {
        server
            .canvas_zoom(Parameters(CanvasZoomParams { level }))
            .await
            .expect("answer");
    }
    assert_eq!(
        fake.calls(),
        vec![
            ("canvas.zoom.reset".to_owned(), serde_json::Value::Null),
            ("canvas.zoom.reset".to_owned(), json!({ "level": 1.5 })),
            ("layout.zoom_to_fit".to_owned(), json!({ "canvas": true })),
        ]
    );
    // Neither a zoom nor `fit`: refused without a command.
    for level in [
        CanvasZoomLevel::Level(0.0),
        CanvasZoomLevel::Level(f64::NAN),
        CanvasZoomLevel::Named("big".to_owned()),
    ] {
        let answer = server
            .canvas_zoom(Parameters(CanvasZoomParams { level: Some(level) }))
            .await
            .expect("answer");
        assert!(answer_json(&answer).0, "refused");
    }
    assert_eq!(fake.calls().len(), 3);
}
