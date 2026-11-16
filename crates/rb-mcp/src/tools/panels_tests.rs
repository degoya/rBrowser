use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use serde_json::json;

use super::{PanelPosition, PanelsDockParams};
use crate::testing::{FakeBackend, answer_json, server};

fn params(position: PanelPosition) -> PanelsDockParams {
    PanelsDockParams {
        position,
        size: None,
        collapsed: None,
        plugin: None,
        panel: None,
    }
}

#[tokio::test]
async fn each_position_runs_its_command() {
    for (position, command) in [
        (PanelPosition::Left, "panels.dock.left"),
        (PanelPosition::Right, "panels.dock.right"),
        (PanelPosition::Bottom, "panels.dock.bottom"),
        (PanelPosition::Floating, "panels.undock"),
        (PanelPosition::Closed, "panels.close"),
    ] {
        let fake = Arc::new(FakeBackend::default());
        server(&fake)
            .panels_dock(Parameters(params(position)))
            .await
            .expect("answer");
        assert_eq!(fake.calls(), [(command.to_owned(), json!({}))]);
    }
}

#[tokio::test]
async fn the_size_and_the_panel_go_with_the_command() {
    let fake = Arc::new(FakeBackend::default());
    fake.answer(
        "panels.dock.bottom",
        Ok(json!({ "position": "bottom", "size": 300 })),
    );
    let answer = server(&fake)
        .panels_dock(Parameters(PanelsDockParams {
            position: PanelPosition::Bottom,
            size: Some(300),
            collapsed: None,
            plugin: Some("network-panel".to_owned()),
            panel: Some("main".to_owned()),
        }))
        .await
        .expect("answer");
    let (error, answer) = answer_json(&answer);
    assert!(!error, "{answer}");
    assert_eq!(answer, json!({ "position": "bottom", "size": 300 }));
    assert_eq!(
        fake.calls(),
        [(
            "panels.dock.bottom".to_owned(),
            json!({ "size": 300, "plugin": "network-panel", "panel": "main" })
        )]
    );
}

#[tokio::test]
async fn a_size_without_a_dock_is_refused_before_any_command() {
    for position in [
        PanelPosition::Floating,
        PanelPosition::Closed,
        PanelPosition::Hidden,
    ] {
        let fake = Arc::new(FakeBackend::default());
        let answer = server(&fake)
            .panels_dock(Parameters(PanelsDockParams {
                size: Some(300),
                ..params(position)
            }))
            .await
            .expect("answer");
        let (error, answer) = answer_json(&answer);
        assert!(error, "{answer}");
        assert!(fake.calls().is_empty());
    }
}

#[test]
fn the_arguments_are_closed_and_named_in_camel_case() {
    let parsed: PanelsDockParams =
        serde_json::from_value(json!({ "position": "floating" })).expect("parses");
    assert_eq!(parsed.position, PanelPosition::Floating);
    assert!(serde_json::from_value::<PanelsDockParams>(json!({ "position": "top" })).is_err());
    assert!(
        serde_json::from_value::<PanelsDockParams>(json!({ "position": "left", "x": 1 })).is_err()
    );
}

#[tokio::test]
async fn collapsed_goes_with_a_dock_and_folds_or_opens_it() {
    for collapsed in [true, false] {
        let fake = Arc::new(FakeBackend::default());
        server(&fake)
            .panels_dock(Parameters(PanelsDockParams {
                collapsed: Some(collapsed),
                ..params(PanelPosition::Right)
            }))
            .await
            .expect("answer");
        assert_eq!(
            fake.calls(),
            [(
                "panels.dock.right".to_owned(),
                json!({ "collapsed": collapsed })
            )]
        );
    }
    for position in [PanelPosition::Floating, PanelPosition::Closed] {
        let fake = Arc::new(FakeBackend::default());
        let answer = server(&fake)
            .panels_dock(Parameters(PanelsDockParams {
                collapsed: Some(true),
                ..params(position)
            }))
            .await
            .expect("answer");
        let (error, answer) = answer_json(&answer);
        assert!(error, "{answer}");
        assert!(fake.calls().is_empty());
    }
}

/// RB-080-93: `hidden` hides the panels - `panels.close` told so - while `closed` leaves it to
/// `panels.closeTo` (the icon rail by default).
#[tokio::test]
async fn hidden_closes_to_hidden() {
    let fake = Arc::new(FakeBackend::default());
    server(&fake)
        .panels_dock(Parameters(params(PanelPosition::Hidden)))
        .await
        .expect("answer");
    assert_eq!(
        fake.calls(),
        [("panels.close".to_owned(), json!({ "to": "hidden" }))]
    );
    let parsed: PanelsDockParams =
        serde_json::from_value(json!({ "position": "hidden" })).expect("parses");
    assert_eq!(parsed.position, PanelPosition::Hidden);
}
