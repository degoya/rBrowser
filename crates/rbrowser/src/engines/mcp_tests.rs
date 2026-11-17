use serde_json::json;

use super::*;
use crate::engines::on_start;
use crate::engines::tests::{setup_with_firefox, until};

#[test]
fn engine_restart_takes_firefox_or_webkit_and_an_optional_view() {
    let args: EngineRestartArgs =
        serde_json::from_value(json!({ "engine": "firefox", "viewId": 3 })).expect("parses");
    assert_eq!(args.engine, ExternalEngine::Firefox);
    assert_eq!(args.view_id, Some(ViewId(3)));
    assert_eq!(EngineKind::from(ExternalEngine::Webkit), EngineKind::Webkit);
    for refused in [
        json!({ "engine": "chromium" }),
        json!({}),
        json!({ "engine": "firefox", "target": "all" }),
    ] {
        assert!(
            serde_json::from_value::<EngineRestartArgs>(refused.clone()).is_err(),
            "{refused}"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn engine_restart_explains_an_unavailable_engine() {
    let core = setup_with_firefox();
    on_start(&core);
    until(|| {
        core.host()
            .shell_state()
            .externals
            .first()
            .is_some_and(|view| view.status == rb_engine::external::ExternalStatus::Unavailable)
    })
    .await;
    let error = engine_restart(&core, &json!({ "engine": "firefox" })).expect_err("unavailable");
    let missing = crate::engines::firefox::reason_of(&rb_bidi::launcher::LocateError::NotFound);
    assert!(error.to_string().contains(&missing), "{error}");
    let error = engine_restart(&core, &json!({ "engine": "webkit" })).expect_err("none open");
    assert!(
        error.to_string().contains("no WebKit view is open"),
        "{error}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn views_list_shows_engine_external_status_and_unsupported() {
    let core = setup_with_firefox();
    on_start(&core);
    until(|| {
        core.host()
            .shell_state()
            .externals
            .first()
            .is_some_and(|view| view.status == rb_engine::external::ExternalStatus::Unavailable)
    })
    .await;
    let grid = view_engine_state(core.host(), ViewId(1)).expect("view 1");
    assert_eq!(
        grid,
        ViewEngineState {
            engine: EngineKind::Chromium,
            external: false,
            status: "running".into(),
            unsupported: Vec::new(),
            external_window: None,
            embedded: None,
        }
    );
    let firefox = view_engine_state(core.host(), ViewId(3)).expect("view 3");
    assert_eq!(firefox.engine, EngineKind::Firefox);
    assert!(firefox.external);
    assert_eq!(firefox.status, "unavailable");
    // The window of an external view: open only while it runs (RB-080-38).
    assert_eq!(firefox.external_window.as_deref(), Some("closed"));
    // In its cell only while its window is bound there (RB-080-39).
    assert_eq!(firefox.embedded, Some(false));
    assert!(view_engine_state(core.host(), ViewId(9)).is_none());
    let json = serde_json::to_value(&firefox).expect("serializes");
    assert_eq!(
        json,
        json!({ "engine": "firefox", "external": true, "status": "unavailable", "unsupported": [], "externalWindow": "closed", "embedded": false })
    );
    let grid_json = serde_json::to_value(&grid).expect("serializes");
    assert!(grid_json.get("externalWindow").is_none(), "{grid_json}");
    assert!(grid_json.get("embedded").is_none(), "{grid_json}");
}
