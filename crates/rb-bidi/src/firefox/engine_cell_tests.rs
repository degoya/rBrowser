//! The Firefox view scaled into its grid cell (RB-080-40).

use rb_engine::EngineKind;
use rb_engine::external::CellScale;
use rb_engine::test_support::conformance::sample_spec;
use rb_engine::{Engine, EngineError, ViewId};
use serde_json::json;

use crate::firefox::test_util::engine;
use crate::testing::FakeFirefox;

/// In its grid cell (RB-080-40) a view's viewport takes the cell's height and keeps the
/// breakpoint's width and DPR; the emulation applied again keeps the cell, and without a cell
/// the breakpoint's own size comes back.
#[tokio::test]
async fn a_cell_scale_sets_the_viewport_to_the_cells_height_and_keeps_the_breakpoint() {
    let fake = FakeFirefox::start().await;
    let engine = engine(&fake, "");
    let mut spec = sample_spec(EngineKind::Firefox, 3);
    spec.emulation.width = 393;
    spec.emulation.height = 852;
    spec.emulation.dpr = 3.0;
    let view = engine.create_view(&spec).await.expect("a view");
    let context = engine.view_state(ViewId(3)).expect("state").context.clone();
    let viewport = |width: u32, height: u32| json!({ "context": context, "viewport": { "width": width, "height": height }, "devicePixelRatio": 3.0 });
    let cell = CellScale {
        factor: 0.82,
        width: 393,
        height: 610,
    };
    view.set_cell(Some(cell)).await.expect("scaled");
    assert_eq!(
        engine.view_state(ViewId(3)).expect("state").cell(),
        Some(cell)
    );
    assert_eq!(
        fake.calls("browsingContext.setViewport").last(),
        Some(&viewport(393, 610))
    );
    view.apply_emulation(&spec.emulation).await.expect("report");
    assert_eq!(
        fake.calls("browsingContext.setViewport").last(),
        Some(&viewport(393, 610)),
        "the emulation applied again keeps the cell"
    );
    view.set_cell(None).await.expect("released");
    assert_eq!(
        fake.calls("browsingContext.setViewport").last(),
        Some(&viewport(393, 852))
    );
    assert_eq!(fake.calls("browsingContext.setViewport").len(), 4);
    view.close().await.expect("closes");
    assert!(matches!(
        view.set_cell(Some(cell)).await,
        Err(EngineError::Closed)
    ));
}
