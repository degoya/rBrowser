use super::*;
use crate::sitemap::render_views::{RenderView, RenderViewMade};

#[test]
fn a_height_is_whole_dip_never_below_the_views_own() {
    assert_eq!(whole(4001.0, 667), 4001);
    assert_eq!(whole(4000.2, 667), 4001);
    assert_eq!(whole(100.0, 667), 667);
    assert_eq!(whole(f64::NAN, 667), 667);
    assert_eq!(whole(-1.0, 667), 667);
    assert_eq!(whole(f64::INFINITY, 667), 667);
}

/// A window that paints every size at once and records the heights asked for.
#[derive(Default)]
struct Painting(std::sync::Mutex<Vec<u32>>);

impl RenderViews for Painting {
    fn open(&self, _view: RenderView, made: RenderViewMade) {
        made(Ok(()));
    }

    fn resize(&self, _marker: String, _width: u32, height: u32, painted: RenderViewMade) {
        crate::cdp::lock(&self.0).push(height);
        painted(Ok(()));
    }

    fn close(&self, _marker: String) {}
}

fn surface(window: &Arc<dyn RenderViews>, width: u32, dpr: f64) -> RenderSurface<'_> {
    RenderSurface {
        window,
        marker: "about:blank#rb-render-1",
        width,
        height: 667,
        dpr,
    }
}

fn heights(window: &Arc<Painting>) -> Vec<u32> {
    crate::cdp::lock(&window.0).clone()
}

#[tokio::test]
async fn a_page_within_the_pixel_budget_grows_the_view() {
    let painting = Arc::new(Painting::default());
    let window: Arc<dyn RenderViews> = painting.clone();
    // A phone at 3: 430 x 5461 DIP is 21 megapixels at its ratio.
    surface(&window, 430, 3.0).fit(5461.0).await.expect("grows");
    assert_eq!(heights(&painting), [5461]);
}

#[tokio::test]
async fn a_page_over_the_pixel_budget_fails_without_growing_the_view() {
    // Security review, wave 090-w23: the grown view and its frames are bounded by area, not by
    // height alone.
    let painting = Arc::new(Painting::default());
    let window: Arc<dyn RenderViews> = painting.clone();
    let error = surface(&window, 1920, 3.0)
        .fit(5461.0)
        .await
        .expect_err("94 megapixels");
    assert!(error.to_string().contains("megapixels"), "{error}");
    let error = surface(&window, 10_000, 1.0)
        .fit(16_384.0)
        .await
        .expect_err("164 megapixels");
    assert!(error.to_string().contains("megapixels"), "{error}");
    assert!(heights(&painting).is_empty());
}

#[tokio::test]
async fn a_height_over_the_device_cap_fails_at_any_pixel_ratio() {
    // A `dpr: 0.001` breakpoint and a page 3e7 px tall: no view of 16 million DIP.
    let painting = Arc::new(Painting::default());
    let window: Arc<dyn RenderViews> = painting.clone();
    for dpr in [0.001, 0.0, f64::NAN, 1.0] {
        let error = surface(&window, 375, dpr)
            .fit(3e7)
            .await
            .expect_err("too tall");
        assert!(error.to_string().contains("too tall"), "{error}");
    }
    assert!(heights(&painting).is_empty());
}

#[test]
fn the_pixel_budget_counts_the_ratio_squared_never_below_one() {
    assert!(within_budget(3840, 16_384, 1.0));
    assert!(within_budget(1920, 8192, 2.0));
    assert!(!within_budget(1920, 5461, 3.0));
    assert!(within_budget(1920, 16_384, 0.001), "counted at 1");
    assert!(!within_budget(10_000, 16_384, f64::NAN));
}
