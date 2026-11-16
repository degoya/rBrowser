//! One visual run over the selected views without the window: each view captured through a
//! [`Shooter`] (CDP for Chromium views), compared with its baseline or stored as the new one,
//! the diff images saved below the screenshot folder. Views go one after another; a view that
//! fails is named in its record and the others go on. The pixel work runs on a blocking thread.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rb_engine::ViewId;
use rb_visual::{
    Baseline, BaselineMeta, BaselineStore, Comparison, DiffOptions, SlugStrategy, Subject,
    Viewport, meta, png_size, save_png, sitemap::file_name, slug,
};

use super::tool::UpdatedBaseline;
use crate::capture::Shot;
use crate::capture::run::Shooter;

/// The settings of Capture > Visual regression in effect.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualOptions {
    /// `visual.threshold`.
    pub threshold: f64,
    /// `visual.fullPage`.
    pub full_page: bool,
    /// `visual.slugStrategy`.
    pub slug: SlugStrategy,
    /// `visual.lfsCheck`.
    pub lfs_check: bool,
    /// `visual.failOnMetaMismatch`.
    pub fail_on_meta_mismatch: bool,
}

/// A view as a run needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewTarget {
    /// The view.
    pub id: ViewId,
    /// Its label.
    pub label: String,
    /// The baseline's name: the breakpoint id, or `<w>x<h>` for a free size.
    pub name: String,
    /// The page it shows.
    pub url: String,
    /// `chromium`, `firefox` or `webkit`.
    pub engine: String,
    /// The breakpoint's viewport in CSS px.
    pub viewport: Viewport,
}

/// What every view of a run shares.
#[derive(Clone, Debug, PartialEq)]
pub struct RunSetup {
    /// The settings.
    pub options: VisualOptions,
    /// This machine and build, as `meta.json` records them (`dpr`, `viewport`, `url`, `slug`
    /// and `fullPage` are filled per view).
    pub machine: BaselineMeta,
    /// Where the diff images go (`<screenshots>/visual/<run>`); `None` saves none.
    pub diff_dir: Option<PathBuf>,
}

impl RunSetup {
    fn shot(&self) -> Shot {
        if self.options.full_page {
            Shot::FullPage
        } else {
            Shot::Viewport
        }
    }

    /// The metadata of `view` captured now as an image `image_width` device pixels wide.
    pub fn meta_of(&self, view: &ViewTarget, slug: &str, image_width: u32) -> BaselineMeta {
        BaselineMeta {
            dpr: meta::pixel_ratio(image_width, view.viewport.width),
            engine: view.engine.clone(),
            url: meta::public_url(&view.url),
            slug: slug.to_owned(),
            viewport: view.viewport,
            full_page: self.options.full_page,
            ..self.machine.clone()
        }
    }
}

/// The baseline file name of each of `views` ([`file_name`]: a breakpoint id that is no plain
/// file name, `iPhone 15`, is stored as `iPhone-15`), or why it has none: another breakpoint of
/// the run maps to the same file. Views of the same breakpoint share theirs.
fn file_names(views: &[ViewTarget]) -> Vec<Result<String, String>> {
    let mut owners: HashMap<String, &str> = HashMap::new();
    views
        .iter()
        .map(|view| {
            let name = file_name(&view.name);
            match owners.get(&name) {
                Some(owner) if *owner != view.name => Err(format!(
                    "{:?} and {owner:?} both map to the baseline file {name}.png",
                    view.name
                )),
                _ => {
                    owners.insert(name.clone(), &view.name);
                    Ok(name)
                }
            }
        })
        .collect()
}

/// The record `view` is compared under (its breakpoint is the file name `name`), and why it
/// cannot have one.
fn subject(
    view: &ViewTarget,
    strategy: SlugStrategy,
    name: &Result<String, String>,
) -> (Subject, Option<String>) {
    let subject = Subject {
        view: view.id.0,
        label: view.label.clone(),
        breakpoint: name
            .as_ref()
            .map_or_else(|_| view.name.clone(), Clone::clone),
        slug: slug(&view.url, strategy),
        url: meta::public_url(&view.url),
    };
    (subject, name.as_ref().err().cloned())
}

/// Compares `views` with their baselines in `store`.
pub async fn compare_views(
    shooter: &dyn Shooter,
    store: &BaselineStore,
    views: &[ViewTarget],
    setup: &RunSetup,
) -> Vec<Comparison> {
    let options = DiffOptions {
        threshold: setup.options.threshold,
        include_aa: false,
    };
    let mut comparisons = Vec::with_capacity(views.len());
    for (view, name) in views.iter().zip(file_names(views)) {
        let (subject, refused) = subject(view, setup.options.slug, &name);
        if let Some(reason) = refused {
            comparisons.push(Comparison::failed(subject, reason));
            continue;
        }
        let png = match shooter.shoot(view.id, &setup.shot(), 1.0).await {
            Ok(png) => png,
            Err(error) => {
                comparisons.push(Comparison::failed(subject, error.to_string()));
                continue;
            }
        };
        let baseline = match store.read(&subject.slug, &subject.breakpoint) {
            Ok(baseline) => baseline,
            Err(error) => {
                comparisons.push(Comparison::failed(subject, error.to_string()));
                continue;
            }
        };
        let width = png_size(&png).map(|(width, _)| width).unwrap_or_default();
        let current = setup.meta_of(view, &subject.slug, width);
        let compared = compare_blocking(subject.clone(), baseline, png.clone(), current, options)
            .await
            .and_then(|mut compared| {
                save_images(setup.diff_dir.as_deref(), &mut compared, &png)?;
                Ok(compared)
            });
        comparisons.push(match compared {
            Ok(compared) => compared.comparison,
            Err(error) => Comparison::failed(subject, error),
        });
    }
    comparisons
}

/// [`rb_visual::compare`] on a blocking thread.
async fn compare_blocking(
    subject: Subject,
    baseline: Option<Baseline>,
    png: Vec<u8>,
    current: BaselineMeta,
    options: DiffOptions,
) -> Result<rb_visual::Compared, String> {
    tokio::task::spawn_blocking(move || {
        rb_visual::compare(subject, baseline.as_ref(), &png, &current, &options)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Saves the diff image and the capture of a regression below `dir` (never over a file), and
/// names them in its record.
fn save_images(
    dir: Option<&Path>,
    compared: &mut rb_visual::Compared,
    png: &[u8],
) -> Result<(), String> {
    let comparison = &mut compared.comparison;
    let Some(dir) = dir.filter(|_| comparison.status.is_regression()) else {
        return Ok(());
    };
    let stem = format!(
        "{}/{}",
        comparison.subject.slug, comparison.subject.breakpoint
    );
    let save = |suffix: &str, bytes: &[u8]| {
        save_png(dir, Path::new(&format!("{stem}-{suffix}")), bytes)
            .map(|path| path.display().to_string())
            .map_err(|error| error.to_string())
    };
    comparison.actual_path = Some(save("actual", png)?);
    if let Some(diff) = &compared.diff_png {
        comparison.diff_path = Some(save("diff", diff)?);
    }
    Ok(())
}

/// Captures `views` and stores them as the new baselines in `store`.
pub async fn update_views(
    shooter: &dyn Shooter,
    store: &BaselineStore,
    views: &[ViewTarget],
    setup: &RunSetup,
) -> Vec<UpdatedBaseline> {
    let mut updated = Vec::with_capacity(views.len());
    for (view, name) in views.iter().zip(file_names(views)) {
        let (subject, refused) = subject(view, setup.options.slug, &name);
        let mut entry = UpdatedBaseline {
            view: subject.view,
            label: subject.label.clone(),
            breakpoint: subject.breakpoint.clone(),
            slug: subject.slug.clone(),
            ..UpdatedBaseline::default()
        };
        let written = match refused {
            Some(reason) => Err(reason),
            None => match shooter.shoot(view.id, &setup.shot(), 1.0).await {
                Err(error) => Err(error.to_string()),
                Ok(png) => match png_size(&png) {
                    None => Err("the capture is no PNG".to_owned()),
                    Some((width, _)) => {
                        let meta = setup.meta_of(view, &subject.slug, width);
                        store
                            .write(&subject.slug, &subject.breakpoint, &png, &meta)
                            .map_err(|error| error.to_string())
                    }
                },
            },
        };
        match written {
            Ok(path) => entry.path = Some(path.display().to_string()),
            Err(error) => entry.error = Some(error),
        }
        updated.push(entry);
    }
    updated
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
