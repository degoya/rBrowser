//! Which design a view shows.
//!
//! 1. The view's breakpoint has an entry in `designs`: that file - or, when the entry is refused
//!    (outside `.rbrowser/designs/`, another type, missing, too large), a [`Problem`] and no image,
//!    never another file in its place.
//! 2. Otherwise the design in `.rbrowser/designs/` (top level) nearest to the view's width in CSS
//!    px: the width a name gives (`home-1440.png`), else the image's width over its export scale
//!    (`@2x`); on a tie the export matching the view's pixel ratio, then the 1x one, then the
//!    name. A file that turns out unusable is skipped.
//!
//! Every file is read through `project.read` and checked ([`crate::image::sniff`]) before its
//! size or a byte of it is used.

use std::collections::BTreeMap;

use rb_plugin_api::{Failure, FailureKind, PluginHost, codes};
use serde_json::{Map, Value, json};

use crate::PLUGIN_ID;
use crate::image::{ImageInfo, MAX_IMAGE_BYTES, Refusal, fingerprint, name_hints, sniff};
use crate::path::{DESIGNS_PROJECT_DIR, DesignPath, MAX_PATH_CHARS, PathRefusal};

/// The most names of `.rbrowser/designs/` the search looks at.
pub const MAX_CANDIDATES: usize = 256;

/// What the view is, as far as the choice goes.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFacts {
    /// The breakpoint id; empty for a free size.
    pub breakpoint: String,
    /// The width in CSS px.
    pub width: f64,
    /// The device pixel ratio.
    pub dpr: f64,
}

/// Why a view shows no image: `code` below `plugin.image-overlay.errors.`, with its parameters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    pub code: &'static str,
    pub params: Vec<(String, String)>,
}

impl Problem {
    fn new(code: &'static str, path: &str) -> Self {
        Self {
            code,
            params: vec![(
                "path".to_owned(),
                path.chars().take(MAX_PATH_CHARS).collect(),
            )],
        }
    }

    /// The i18n key of the message.
    #[must_use]
    pub fn key(&self) -> String {
        format!("plugin.{PLUGIN_ID}.errors.{}", self.code)
    }

    /// As the panel and MCP get it: `{"code", "params"}`.
    #[must_use]
    pub fn json(&self) -> Value {
        let params: Map<String, Value> = self
            .params
            .iter()
            .map(|(name, value)| (name.clone(), json!(value)))
            .collect();
        json!({ "code": self.key(), "params": params })
    }

    /// As a call's failure.
    #[must_use]
    pub fn failure(&self) -> Failure {
        let kind = match self.code {
            "not_found" => FailureKind::NotFound,
            "path_outside" => FailureKind::AccessDenied,
            "too_large" | "too_many_pixels" => FailureKind::Limit,
            _ => FailureKind::BadData,
        };
        self.params.iter().fold(
            Failure::coded(kind, &self.key()),
            |failure, (name, value)| failure.with_param(name, value),
        )
    }
}

/// The design a view shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Chosen {
    pub path: DesignPath,
    pub info: ImageInfo,
    /// The file's length in bytes.
    pub bytes: usize,
    /// Its [`fingerprint`].
    pub hash: String,
    /// How many image px make one CSS px: the export scale with `autoScaleDpr`, else 1.
    pub scale: u32,
}

/// The outcome of [`choose`].
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    Image(Chosen),
    /// No design for this view (none mapped, none in the folder, no project).
    None,
    Problem(Problem),
}

/// The design path `text` names, checked.
///
/// # Errors
///
/// `path_outside` or `bad_type`.
pub fn design_path(text: &str) -> Result<DesignPath, Problem> {
    DesignPath::parse(text).map_err(|refusal| match refusal {
        PathRefusal::Outside => Problem::new("path_outside", text),
        PathRefusal::BadType => Problem::new("bad_type", text),
    })
}

/// Reads `path` and checks it is the image its extension names, within the limits.
///
/// # Errors
///
/// `not_found`, `too_large`, `unreadable` (the host's code as `reason`), `bad_type`,
/// `too_many_pixels`.
pub fn read_design(
    host: &mut dyn PluginHost,
    path: &DesignPath,
) -> Result<(Vec<u8>, ImageInfo), Problem> {
    let bytes = host.project_read(&path.project_path()).map_err(|failure| {
        if failure.kind == FailureKind::NotFound {
            Problem::new("not_found", path.as_str())
        } else if failure.code.as_deref() == Some(codes::TOO_LARGE) {
            Problem::new("too_large", path.as_str())
        } else {
            let mut problem = Problem::new("unreadable", path.as_str());
            let reason = failure.code.unwrap_or_default();
            problem.params.push(("reason".to_owned(), reason));
            problem
        }
    })?;
    let info = sniff(&bytes, path.kind()).map_err(|refusal| match refusal {
        Refusal::BadType => Problem::new("bad_type", path.as_str()),
        Refusal::TooLarge => Problem::new("too_large", path.as_str()),
        Refusal::TooManyPixels => Problem::new("too_many_pixels", path.as_str()),
    })?;
    Ok((bytes, info))
}

/// Reads `path` for a page's chunk, held to the fingerprint the page started with.
///
/// # Errors
///
/// As [`read_design`], or `changed` when the file is no longer the one fingerprinted.
pub fn read_checked(
    host: &mut dyn PluginHost,
    path: &DesignPath,
    hash: &str,
) -> Result<Vec<u8>, Problem> {
    let (bytes, _) = read_design(host, path)?;
    if fingerprint(&bytes) == hash {
        Ok(bytes)
    } else {
        Err(Problem::new("changed", path.as_str()))
    }
}

/// `path` as the design a view shows: its fingerprint, length and export scale.
pub(crate) fn chosen(path: DesignPath, bytes: &[u8], info: ImageInfo, auto_scale: bool) -> Chosen {
    let scale = if auto_scale {
        name_hints(path.as_str()).1
    } else {
        1
    };
    debug_assert!(bytes.len() <= MAX_IMAGE_BYTES);
    Chosen {
        hash: fingerprint(bytes),
        bytes: bytes.len(),
        path,
        info,
        scale,
    }
}

/// The design `view` shows with the mapping `designs`.
pub fn choose(
    host: &mut dyn PluginHost,
    designs: &BTreeMap<String, String>,
    auto_scale: bool,
    view: &ViewFacts,
) -> Choice {
    let mapped = (!view.breakpoint.is_empty())
        .then(|| designs.get(&view.breakpoint))
        .flatten();
    let Some(text) = mapped else {
        return nearest(host, auto_scale, view);
    };
    let outcome = design_path(text).and_then(|path| {
        let (bytes, info) = read_design(host, &path)?;
        Ok(chosen(path, &bytes, info, auto_scale))
    });
    match outcome {
        Ok(chosen) => Choice::Image(chosen),
        Err(problem) => {
            host.log(
                "warn",
                &format!("design of {} refused: {}", view.breakpoint, problem.code),
            );
            Choice::Problem(problem)
        }
    }
}

/// The ratio an export of `scale` fits best for `view`: 0 its pixel ratio, 1 the 1x one, 2 other.
fn preference(scale: u32, auto_scale: bool, view: &ViewFacts) -> u8 {
    let ratio = if view.dpr.is_finite() {
        view.dpr.round().clamp(1.0, 4.0)
    } else {
        1.0
    };
    if auto_scale && f64::from(scale) == ratio {
        0
    } else if scale == 1 {
        1
    } else {
        2
    }
}

fn nearest(host: &mut dyn PluginHost, auto_scale: bool, view: &ViewFacts) -> Choice {
    let Ok(names) = host.project_list(DESIGNS_PROJECT_DIR) else {
        return Choice::None;
    };
    let mut ranked: Vec<(f64, u8, DesignPath)> = Vec::new();
    for name in names.iter().take(MAX_CANDIDATES) {
        let Some(path) = DesignPath::listed(name) else {
            continue;
        };
        let (hinted, scale) = name_hints(path.as_str());
        let width = match hinted {
            Some(width) => f64::from(width),
            None => match read_design(host, &path) {
                Ok((_, info)) => f64::from(info.width) / f64::from(scale),
                Err(problem) => {
                    host.log("warn", &format!("design skipped: {}", problem.code));
                    continue;
                }
            },
        };
        let distance = (width - view.width).abs();
        ranked.push((distance, preference(scale, auto_scale, view), path));
    }
    ranked.sort_by(|a, b| {
        a.0.total_cmp(&b.0)
            .then(a.1.cmp(&b.1))
            .then_with(|| a.2.as_str().cmp(b.2.as_str()))
    });
    for (_, _, path) in ranked {
        match read_design(host, &path) {
            Ok((bytes, info)) => return Choice::Image(chosen(path, &bytes, info, auto_scale)),
            Err(problem) => host.log("warn", &format!("design skipped: {}", problem.code)),
        }
    }
    Choice::None
}

#[cfg(test)]
#[path = "select_tests.rs"]
mod tests;
