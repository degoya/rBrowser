//! One capture of one Chromium view over its flat CDP session (ADR 0001): never a read of the
//! screen. rBrowser owns `Emulation.setDeviceMetricsOverride`, so a capture that needs another
//! viewport height (the breakpoint's own height instead of the cell's, the page's for
//! `fullPage`) sets it for the capture only and then sends the host's metrics again - also when
//! the capture failed.
//!
//! - `viewport`: the visual viewport at the breakpoint's size, `clip` from
//!   `Page.getLayoutMetrics` (a clip renders at the emulated DPR, not at the cell's scale).
//! - `fullPage`: the viewport height raised to `cssContentSize.height` (at most
//!   [`MAX_DEVICE_PX`] device pixels), `captureBeyondViewport`.
//! - `element`: `DOM.querySelector` (or a picked node), `DOM.getBoxModel`'s border box plus
//!   the padding, moved into the page by the scroll offset, `captureBeyondViewport`, so an
//!   element out of the viewport is captured as well.
//!
//! A windowless render view (RB-090-83) paints only its own rectangle: with the page's height in
//! the emulation alone its full-page image repeated the top viewport down the page (RB-090-88).
//! Its [`Surface`] grows the view to the page's height after the override and before the shot,
//! and gives it its own size back after the restore ([`capture_on`]); a grid view is a window
//! and needs none ([`FixedSurface`]).

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use rb_engine::EngineError;
use serde_json::{Value, json};

use crate::cdp::CdpRouter;

/// The longest side of a capture in device pixels (a GPU texture); a taller page is cut.
pub const MAX_DEVICE_PX: f64 = 16_384.0;
/// The longest selector taken.
pub const MAX_SELECTOR_LEN: usize = 1024;

/// What a capture needs of a view, from the host (`Host::capture_target`).
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureTarget {
    /// The view's flat CDP session.
    pub session: String,
    /// The parameters of `Emulation.setDeviceMetricsOverride` the host sent last.
    pub metrics: Value,
    /// The breakpoint's viewport height in CSS px (`0`: the cell's).
    pub height: u32,
    /// The device pixel ratio (`0`: the display's own).
    pub dpr: f64,
    /// The view is hidden (out of sight) and throttled.
    pub hidden: bool,
}

/// Which part of the page.
#[derive(Clone, Debug, PartialEq)]
pub enum Shot {
    /// The viewport at the breakpoint's size.
    Viewport,
    /// The whole page.
    FullPage,
    /// One element, found by `selector`, with `padding` CSS px around it.
    Element {
        /// A CSS selector.
        selector: String,
        /// CSS px around the element.
        padding: u32,
    },
    /// One element the user picked (`Overlay.inspectNodeRequested`).
    Picked {
        /// The node.
        backend_node_id: i64,
        /// CSS px around the element.
        padding: u32,
    },
}

/// The `clip` of `Page.captureScreenshot`, in CSS px of the page.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Clip {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
    /// The scale of the output (`1`: the emulated DPR; `1 / dpr`: CSS px).
    pub scale: f64,
}

impl Clip {
    fn to_json(self) -> Value {
        json!({
            "x": self.x,
            "y": self.y,
            "width": self.width,
            "height": self.height,
            "scale": self.scale,
        })
    }
}

/// The clip of an element: the bounding box of its border quad (viewport coordinates) moved by
/// the page offset of the layout viewport, `padding` on every side. A box that would start
/// left of or above the page is moved in, never shrunk, so the image keeps its size.
pub fn element_clip(quad: &[f64], padding: u32, page: (f64, f64), scale: f64) -> Option<Clip> {
    if quad.len() != 8 || quad.iter().any(|value| !value.is_finite()) {
        return None;
    }
    let xs = [quad[0], quad[2], quad[4], quad[6]];
    let ys = [quad[1], quad[3], quad[5], quad[7]];
    let left = xs.iter().copied().fold(f64::INFINITY, f64::min);
    let right = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let top = ys.iter().copied().fold(f64::INFINITY, f64::min);
    let bottom = ys.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let padding = f64::from(padding);
    Some(Clip {
        x: (left + page.0 - padding).max(0.0),
        y: (top + page.1 - padding).max(0.0),
        width: right - left + 2.0 * padding,
        height: bottom - top + 2.0 * padding,
        scale,
    })
}

/// The CSS height of a full-page capture: the page's, at least the viewport's, at most
/// [`MAX_DEVICE_PX`] device pixels - and at most that many CSS px at a pixel ratio below 1.
pub fn full_page_height(content: f64, viewport: f64, dpr: f64, scale: f64) -> f64 {
    // A ratio below 1 (or none) counts as 1: the cap in CSS px is never above the device one.
    let device = dpr.max(1.0) * scale.max(f64::EPSILON);
    content
        .max(viewport)
        .ceil()
        .min((MAX_DEVICE_PX / device).floor())
}

/// `params` with another `height`.
fn with_height(params: &Value, height: f64) -> Value {
    let mut params = params.clone();
    if let Some(object) = params.as_object_mut() {
        object.insert("height".to_owned(), json!(height));
    }
    params
}

fn number(value: &Value, pointer: &str) -> Result<f64, EngineError> {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .ok_or_else(|| EngineError::Invalid(format!("CDP answered without {pointer}")))
}

/// What a full-page capture does to the view's painted area besides the emulation.
#[async_trait]
pub trait Surface: Send + Sync {
    /// Grows the view to `height` CSS px and waits (bounded) for its frame at that size.
    ///
    /// # Errors
    ///
    /// The view cannot be resized (it is gone) or painted no frame that tall: a frame of the old
    /// size is never captured as the full page.
    async fn fit(&self, height: f64) -> Result<(), EngineError>;
    /// Gives the view its own size back; after every [`Surface::fit`], also a failed one.
    async fn restore(&self);
}

/// The surface of a view in a window: it paints the whole emulated page already.
pub struct FixedSurface;

#[async_trait]
impl Surface for FixedSurface {
    async fn fit(&self, _height: f64) -> Result<(), EngineError> {
        Ok(())
    }

    async fn restore(&self) {}
}

/// Captures `shot` of the view behind `target` as PNG bytes at `scale`. `restore` gives the
/// metrics to send back afterwards (the host's at that moment; `None`: the view is gone).
pub async fn capture(
    cdp: &Arc<CdpRouter>,
    target: &CaptureTarget,
    shot: &Shot,
    scale: f64,
    restore: &(dyn Fn() -> Option<Value> + Send + Sync),
) -> Result<Vec<u8>, EngineError> {
    capture_on(cdp, target, shot, scale, restore, &FixedSurface).await
}

/// [`capture`] of a view whose painted area is `surface`: a full page grows it to the page's
/// height after the override and before the shot, and shrinks it after the restore - also when
/// the capture failed.
pub async fn capture_on(
    cdp: &Arc<CdpRouter>,
    target: &CaptureTarget,
    shot: &Shot,
    scale: f64,
    restore: &(dyn Fn() -> Option<Value> + Send + Sync),
    surface: &dyn Surface,
) -> Result<Vec<u8>, EngineError> {
    let session = Some(target.session.as_str());
    let current = number(&target.metrics, "/height")?;
    let nominal_height = if target.height > 0 {
        f64::from(target.height)
    } else {
        current
    };
    // Set before each override call: one that failed may still have taken effect.
    let mut overridden = false;
    // Set before the surface grows: a fit that failed may still have resized it.
    let mut grown = false;
    let mut result = Ok(());
    if (nominal_height - current).abs() > f64::EPSILON {
        overridden = true;
        result = cdp
            .call(
                "Emulation.setDeviceMetricsOverride",
                with_height(&target.metrics, nominal_height),
                session,
            )
            .await
            .map(|_| ());
    }
    let result = match result {
        Ok(()) => {
            let changed = Changed {
                overridden: &mut overridden,
                grown: &mut grown,
            };
            shoot(cdp, target, shot, scale, nominal_height, changed, surface).await
        }
        Err(error) => Err(error),
    };
    // Back to the host's metrics whatever happened; a failed restore wins over the image (the
    // caller then has the host send everything again).
    let restored = match restore().filter(|_| overridden) {
        Some(params) => cdp
            .call("Emulation.setDeviceMetricsOverride", params, session)
            .await
            .map(|_| ()),
        None => Ok(()),
    };
    if grown {
        surface.restore().await;
    }
    restored.and(result)
}

/// What a shot changed of the view, for [`capture_on`] to put back.
struct Changed<'a> {
    overridden: &'a mut bool,
    grown: &'a mut bool,
}

async fn shoot(
    cdp: &Arc<CdpRouter>,
    target: &CaptureTarget,
    shot: &Shot,
    scale: f64,
    nominal_height: f64,
    changed: Changed<'_>,
    surface: &dyn Surface,
) -> Result<Vec<u8>, EngineError> {
    let session = Some(target.session.as_str());
    let width = number(&target.metrics, "/width")?;
    let metrics = cdp
        .call("Page.getLayoutMetrics", json!({}), session)
        .await?;
    let (clip, beyond) = match shot {
        Shot::Viewport => {
            let clip = Clip {
                x: number(&metrics, "/cssVisualViewport/pageX")?,
                y: number(&metrics, "/cssVisualViewport/pageY")?,
                width,
                height: nominal_height,
                scale,
            };
            (clip, false)
        }
        Shot::FullPage => {
            let content = number(&metrics, "/cssContentSize/height")?;
            let height = full_page_height(content, nominal_height, target.dpr, scale);
            *changed.overridden = true;
            cdp.call(
                "Emulation.setDeviceMetricsOverride",
                with_height(&target.metrics, height),
                session,
            )
            .await?;
            // The frame the shot copies is painted at the view's size: the page's height too.
            *changed.grown = true;
            surface.fit(height).await?;
            let clip = Clip {
                x: 0.0,
                y: 0.0,
                width,
                height,
                scale,
            };
            (clip, true)
        }
        Shot::Element { selector, padding } => {
            let node = query(cdp, session, selector).await?;
            let model = cdp
                .call("DOM.getBoxModel", json!({ "nodeId": node }), session)
                .await?;
            (element(&metrics, &model, *padding, scale)?, true)
        }
        Shot::Picked {
            backend_node_id,
            padding,
        } => {
            let model = cdp
                .call(
                    "DOM.getBoxModel",
                    json!({ "backendNodeId": backend_node_id }),
                    session,
                )
                .await?;
            (element(&metrics, &model, *padding, scale)?, true)
        }
    };
    let answer = cdp
        .call(
            "Page.captureScreenshot",
            json!({
                "format": "png",
                "clip": clip.to_json(),
                "captureBeyondViewport": beyond,
                "fromSurface": true,
            }),
            session,
        )
        .await?;
    let data = answer
        .get("data")
        .and_then(Value::as_str)
        .ok_or_else(|| EngineError::Invalid("captureScreenshot gave no data".to_owned()))?;
    base64::engine::general_purpose::STANDARD
        .decode(data)
        .map_err(|error| EngineError::Invalid(format!("captureScreenshot data: {error}")))
}

/// The node `selector` names in the main frame.
async fn query(
    cdp: &Arc<CdpRouter>,
    session: Option<&str>,
    selector: &str,
) -> Result<i64, EngineError> {
    let document = cdp
        .call("DOM.getDocument", json!({ "depth": 0 }), session)
        .await?;
    let root = document
        .pointer("/root/nodeId")
        .and_then(Value::as_i64)
        .ok_or_else(|| EngineError::Invalid("DOM.getDocument gave no root".to_owned()))?;
    let found = cdp
        .call(
            "DOM.querySelector",
            json!({ "nodeId": root, "selector": selector }),
            session,
        )
        .await?;
    match found.get("nodeId").and_then(Value::as_i64) {
        Some(node) if node > 0 => Ok(node),
        _ => Err(EngineError::Invalid(format!(
            "no element matches {selector}"
        ))),
    }
}

fn element(metrics: &Value, model: &Value, padding: u32, scale: f64) -> Result<Clip, EngineError> {
    let quad: Vec<f64> = model
        .pointer("/model/border")
        .and_then(Value::as_array)
        .map(|values| values.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let page = (
        number(metrics, "/cssLayoutViewport/pageX")?,
        number(metrics, "/cssLayoutViewport/pageY")?,
    );
    element_clip(&quad, padding, page, scale)
        .ok_or_else(|| EngineError::Invalid("the element has no box".to_owned()))
}

#[cfg(test)]
#[path = "cdp_tests.rs"]
mod tests;
