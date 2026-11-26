//! One capture run over the selected views, in layout order: each view's image to a file (the
//! path pattern below the output folder) or into the answer as base64, or one overview image of
//! all of them. Views are captured one after another; a view that fails is named in the answer
//! and the others go on. The capture itself is a [`Shooter`] (CDP for Chromium views,
//! `chromium.rs`), so the run is tested without a browser.
//!
//! A full page cut at the image's limits (RB-099-11) is named in its entry (`truncated`, the
//! page's height), read from the PNG the stitcher marked; the overview keeps the top
//! [`OVERVIEW_ROWS`] rows of each view and marks a longer one the same way. A full page taken
//! while images were still loading at the lazy-loading pre-pass's deadline says so
//! (`imagesPending`, RB-099-28), read from the PNG's note the same way.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use base64::Engine as _;
use rb_engine::{EngineError, ViewId};
use rb_visual::{
    OverviewOptions, PathVars, Stamp, Tile, compose, decode_top, encode_png, expand,
    images_pending, png_size, save_png, size_text, truncation,
};

use super::CaptureError;
use super::cdp::Shot;
use super::tool::{CaptureMode, MAX_INLINE_BYTES, ScreenshotAnswer, ShotAnswer};

/// The most rows of one view in the overview: one texture, as tall as a full page was before
/// RB-099-11; a longer page is cut there, never decoded whole.
pub const OVERVIEW_ROWS: u32 = 16_384;

/// Takes the picture of one view.
#[async_trait]
pub trait Shooter: Send + Sync {
    /// `shot` of `view` as PNG bytes; `scale` `1` renders at the emulated DPR, `1 / dpr` at CSS
    /// pixels (the overview).
    async fn shoot(&self, view: ViewId, shot: &Shot, scale: f64) -> Result<Vec<u8>, EngineError>;
}

/// What a run knows of a view.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewMeta {
    /// The view.
    pub id: ViewId,
    /// Its label.
    pub label: String,
    /// `{breakpoint}`: the breakpoint id, or `<w>x<h>` for a free size.
    pub name: String,
    /// The breakpoint's width in CSS px.
    pub width: u32,
    /// The breakpoint's height in CSS px.
    pub height: u32,
    /// The device pixel ratio (`0`: the display's own).
    pub dpr: f64,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
}

/// What every view of a run shares.
#[derive(Clone, Debug, PartialEq)]
pub struct RunContext {
    /// The environment id.
    pub env: Option<String>,
    /// The environment's colour, the overview's frame.
    pub env_color: Option<[u8; 3]>,
    /// `capture.pathPattern`.
    pub pattern: String,
    /// `capture.overviewLabels`.
    pub overview_labels: bool,
    /// The output folder; `None`: base64 in the answer.
    pub folder: Option<PathBuf>,
    /// When the run started.
    pub stamp: Stamp,
}

impl RunContext {
    fn vars(&self, view: &ViewMeta, mode: &str) -> PathVars {
        PathVars {
            stamp: self.stamp,
            env: self.env.clone(),
            breakpoint: view.name.clone(),
            view: view.id.0,
            mode: mode.to_owned(),
            width: view.width,
            height: view.height,
            dpr: view.dpr,
        }
    }
}

/// Runs `shot` over `views` (in layout order): a file or base64 each, or with `overview` one
/// image of all of them.
pub async fn run(
    shooter: &dyn Shooter,
    views: &[ViewMeta],
    shot: &Shot,
    mode: CaptureMode,
    overview: bool,
    context: &RunContext,
) -> Result<ScreenshotAnswer, CaptureError> {
    if views.is_empty() {
        return Err(CaptureError::NoViews);
    }
    let mut inline = 0;
    let mut answer = ScreenshotAnswer::default();
    let mut tiles = Vec::new();
    for view in views {
        let scale = if overview && view.dpr > 0.0 {
            1.0 / view.dpr
        } else {
            1.0
        };
        let mut entry = ShotAnswer {
            view: Some(view.id),
            label: view.label.clone(),
            ..ShotAnswer::default()
        };
        match shooter.shoot(view.id, shot, scale).await {
            Ok(png) if overview => match decode_top(&png, OVERVIEW_ROWS) {
                Ok((image, height)) => {
                    (entry.width, entry.height) = (image.width(), image.height());
                    note_cut(&mut entry, &png);
                    entry.truncated |= height > image.height();
                    tiles.push(tile(view, context, image));
                }
                Err(error) => entry.error = Some(error.to_string()),
            },
            Ok(png) => {
                (entry.width, entry.height) = png_size(&png).unwrap_or_default();
                note_cut(&mut entry, &png);
                let vars = context.vars(view, mode.as_str());
                deliver(&mut entry, &png, &vars, context, &mut inline)?;
            }
            Err(error) => entry.error = Some(error.to_string()),
        }
        answer.shots.push(entry);
    }
    if overview && !tiles.is_empty() {
        let options = OverviewOptions {
            labels: context.overview_labels,
        };
        let image = compose(&tiles, &options)?;
        let png = encode_png(&image)?;
        let mut entry = ShotAnswer {
            label: "overview".to_owned(),
            width: image.width(),
            height: image.height(),
            ..ShotAnswer::default()
        };
        let whole = ViewMeta {
            id: ViewId(0),
            label: entry.label.clone(),
            name: entry.label.clone(),
            width: image.width(),
            height: image.height(),
            dpr: 0.0,
            engine: String::new(),
        };
        deliver(
            &mut entry,
            &png,
            &context.vars(&whole, "overview"),
            context,
            &mut inline,
        )?;
        answer.overview = Some(entry);
    }
    if answer.overview.is_none() && answer.shots.iter().all(|shot| shot.error.is_some()) {
        let first = answer.shots.first().and_then(|shot| shot.error.clone());
        return Err(CaptureError::Failed(first.unwrap_or_default()));
    }
    answer.folder = answer
        .paths()
        .first()
        .and_then(|path| Path::new(path).parent())
        .map(|folder| folder.display().to_string());
    Ok(answer)
}

/// Marks `entry` cut when the stitcher recorded `png` as the top of a longer page, and with
/// images pending when the capture's lazy-loading pre-pass ran out of its deadline (RB-099-28).
fn note_cut(entry: &mut ShotAnswer, png: &[u8]) {
    if let Some(cut) = truncation(png) {
        entry.truncated = true;
        entry.page_height = Some(cut.page_height);
    }
    entry.images_pending = images_pending(png);
}

/// The tile of `view` in the overview.
fn tile(view: &ViewMeta, context: &RunContext, image: rb_visual::Rgba) -> Tile {
    let detail = match &context.env {
        Some(env) => format!("{} \u{b7} {env}", view.engine),
        None => view.engine.clone(),
    };
    Tile {
        label: view.label.clone(),
        size: size_text(view.width, view.height, view.dpr),
        detail,
        color: context.env_color,
        image,
    }
}

/// Writes `png` where the pattern says, or puts it into `entry` as base64.
fn deliver(
    entry: &mut ShotAnswer,
    png: &[u8],
    vars: &PathVars,
    context: &RunContext,
    inline: &mut usize,
) -> Result<(), CaptureError> {
    match &context.folder {
        Some(folder) => {
            let relative = expand(&context.pattern, vars)?;
            let path = save_png(folder, &relative, png)?;
            entry.path = Some(path.display().to_string());
        }
        None => {
            *inline += png.len();
            if *inline > MAX_INLINE_BYTES {
                return Err(CaptureError::Invalid(format!(
                    "more than {MAX_INLINE_BYTES} bytes of PNG; save them as files instead"
                )));
            }
            entry.png_base64 = Some(base64::engine::general_purpose::STANDARD.encode(png));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
