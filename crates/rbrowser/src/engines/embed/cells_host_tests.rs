//! The views scaled to their cells, as the host's engines drive it (RB-080-40): the window
//! measures, the plan sends, a replaced window is bound again, nothing while it is off.

use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_engine::external::{CellPixels, CellScale, ExternalStatus};

use super::cells::SETTLE;
use super::tests::{embeds, running_on, wait};
use crate::cdp::lock;
use crate::engines::tests::until;

/// The window measures the cell in pixels (RB-080-40): a WebKit view gets its own scale - the
/// breakpoint's width at the cell's factor, the cell's height - once, and again only for a
/// changed cell; released, it goes back to the breakpoint's size; a restarted view opens with
/// the cell's scale in its spec.
#[tokio::test(flavor = "multi_thread")]
async fn a_measured_cell_scales_the_webkit_view_into_it() {
    let (setup, engines, webkit) = running_on(EngineKind::Webkit, Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let view = webkit.view(ViewId(3)).expect("opened");
    assert_eq!(
        view.cells(),
        Vec::<Option<CellScale>>::new(),
        "nothing measured yet"
    );
    // The mobile breakpoint is 393 wide; a cell of 315 x 500 px: factor 0.8.
    let pixels = CellPixels {
        width: 315,
        height: 500,
    };
    engines.cell_sized(ViewId(3), pixels);
    let scaled = Some(CellScale {
        factor: 0.8,
        width: 393,
        height: 625,
    });
    until(|| view.cells() == [scaled]).await;
    engines.cell_sized(ViewId(3), pixels);
    engines.publish();
    assert_eq!(view.cells(), [scaled], "sent once");
    // A Chromium cell is not the engines' business.
    engines.cell_sized(ViewId(1), pixels);
    assert_eq!(engines.firefox_render_scale(), None, "no Firefox view");
    // Released: the breakpoint's own size.
    engines.release(ViewId(3));
    until(|| view.cells() == [scaled, None]).await;
    engines.embed_again(ViewId(3));
    until(|| view.cells() == [scaled, None, scaled]).await;
    // A restart opens with the scale in its spec.
    engines
        .windows()
        .restart(EngineKind::Webkit, Some(ViewId(3)))
        .expect("restarts");
    until(|| webkit.views().len() == 2).await;
    until(|| engines.list()[0].status == ExternalStatus::Running).await;
    let spec = webkit.views()[1].spec();
    assert_eq!(spec.cell, scaled);
}

/// A view whose engine built its window anew for the scale (a WebKit context, RB-080-40) is
/// asked into its cell again.
#[tokio::test(flavor = "multi_thread")]
async fn a_replaced_window_is_asked_into_its_cell_again() {
    let (setup, engines, webkit) = running_on(EngineKind::Webkit, Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    assert!(engines.list()[0].embedded);
    webkit.view(ViewId(3)).expect("opened").replace_window();
    until(|| embeds(&setup).len() == 2).await;
    assert!(
        !engines.list()[0].embedded,
        "not in its cell until the window says so"
    );
    engines.embedded_bound(ViewId(3));
    assert!(engines.list()[0].embedded);
}

/// A cell measured several times while the person drags (review finding 2) is sent once,
/// after it settled: never an intermediate factor, so neither Firefox nor a WebKit context
/// is built anew for every step.
#[tokio::test(flavor = "multi_thread")]
async fn a_cell_settles_before_its_scale_is_sent() {
    let (setup, engines, webkit) = running_on(EngineKind::Webkit, Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let view = webkit.view(ViewId(3)).expect("opened");
    for width in [315, 300, 280] {
        engines.cell_sized(ViewId(3), CellPixels { width, height: 500 });
    }
    let settled = Some(CellScale {
        factor: 0.7,
        width: 393,
        height: 714,
    });
    wait("the settled cell", || !view.cells().is_empty()).await;
    assert_eq!(view.cells(), [settled], "only the settled cell");
}

/// A scale the view refused (a timeout, a window on its way out) is sent again with the next
/// publish, not only with the next cell (review finding 5).
#[tokio::test(flavor = "multi_thread")]
async fn a_scale_the_view_refused_is_sent_again() {
    let (setup, engines, webkit) = running_on(EngineKind::Webkit, Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let view = webkit.view(ViewId(3)).expect("opened");
    view.fail_next_cell("busy");
    engines.cell_sized(
        ViewId(3),
        CellPixels {
            width: 315,
            height: 500,
        },
    );
    wait("the refused scale", || view.cells().len() == 1).await;
    wait("the refusal forgotten", || {
        !lock(&engines.embedding)
            .applied_cells
            .contains_key(&ViewId(3))
    })
    .await;
    assert!(
        lock(&engines.embedding).settled.contains_key(&ViewId(3)),
        "the cell is kept"
    );
    wait("the scale again", || {
        engines.publish();
        view.cells().len() == 2
    })
    .await;
    assert_eq!(view.cells()[1], view.cells()[0]);
}

/// With the embedding off, or the scaling off (`engines.external.scaleToCell`), a measured cell
/// is kept but nothing is sent to the views (review findings 9 and 10).
#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_sent_while_the_embedding_or_the_scaling_is_off() {
    let (setup, engines, webkit) = running_on(EngineKind::Webkit, Some(4242)).await;
    until(|| embeds(&setup).len() == 1).await;
    engines.embedded_bound(ViewId(3));
    let view = webkit.view(ViewId(3)).expect("opened");
    let pixels = CellPixels {
        width: 315,
        height: 500,
    };
    let mut settings = setup.core.user_settings().engines.external;
    settings.embed = true;
    settings.scale_to_cell = false;
    engines.apply_settings(&settings);
    engines.cell_sized(ViewId(3), pixels);
    tokio::time::sleep(SETTLE * 3).await;
    engines.publish();
    assert_eq!(view.cells(), Vec::<Option<CellScale>>::new(), "scaling off");
    settings.embed = false;
    settings.scale_to_cell = true;
    engines.apply_settings(&settings);
    engines.cell_sized(
        ViewId(3),
        CellPixels {
            width: 280,
            height: 500,
        },
    );
    tokio::time::sleep(SETTLE * 3).await;
    engines.publish();
    assert_eq!(
        view.cells(),
        Vec::<Option<CellScale>>::new(),
        "embedding off"
    );
    // On again: the cell measured meanwhile is sent.
    settings.embed = true;
    engines.apply_settings(&settings);
    until(|| view.cells().len() == 1).await;
    assert_eq!(
        view.cells()[0],
        Some(CellScale {
            factor: 0.7,
            width: 393,
            height: 714,
        })
    );
}
