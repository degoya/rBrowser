//! The breakpoints (RB-070-13): the font size of each text role in every view, ordered by width,
//! with jumps between neighbouring breakpoints marked, and the fluid-typography rule - a heading
//! that does not grow by `fonts.fluidMinRatio` from the narrowest to the widest view is "not
//! scaled".

use rb_plugin_api::{Finding, ViewId};
use serde_json::{Value, json};

use crate::rules::{self, At, Sink, shown};
use crate::run::ViewFonts;
use crate::settings::Settings;

/// The roles the rule holds to scaling.
pub const FLUID_ROLES: &[&str] = &["h1", "h2", "h3"];
/// Views at most this much wider than each other are too close to judge scaling.
pub const MIN_WIDTH_RATIO: f64 = 1.5;
/// A size change between neighbouring breakpoints beyond this factor is a jump.
pub const JUMP_RATIO: f64 = 1.5;

/// One view's sizes of one role.
#[derive(Clone, Debug, PartialEq)]
pub struct Point<'a> {
    pub view: &'a ViewFonts,
    pub width: f64,
    pub min: f64,
    pub max: f64,
}

/// One role across the views, narrowest first.
#[derive(Clone, Debug, PartialEq)]
pub struct Series<'a> {
    pub role: String,
    pub points: Vec<Point<'a>>,
}

impl Series<'_> {
    /// The largest size of the widest view divided by that of the narrowest; `None` when the
    /// views are too close in width (or only one shows the role).
    #[must_use]
    pub fn growth(&self) -> Option<f64> {
        let (narrow, wide) = (self.points.first()?, self.points.last()?);
        if narrow.width <= 0.0 || wide.width / narrow.width < MIN_WIDTH_RATIO || narrow.max <= 0.0 {
            return None;
        }
        Some(wide.max / narrow.max)
    }

    /// A size change between neighbouring breakpoints beyond [`JUMP_RATIO`].
    #[must_use]
    pub fn jump(&self) -> bool {
        self.points.windows(2).any(|pair| {
            let (a, b) = (pair[0].max, pair[1].max);
            a > 0.0 && b > 0.0 && (b / a > JUMP_RATIO || a / b > JUMP_RATIO)
        })
    }
}

/// The roles of `views`, each with a point per view showing it, narrowest first; in the order the
/// roles first appear.
#[must_use]
pub fn series<'a>(views: &[&'a ViewFonts]) -> Vec<Series<'a>> {
    let mut list: Vec<Series<'a>> = Vec::new();
    for view in views {
        for role in &view.fonts.roles {
            let point = Point {
                view,
                width: view.width(),
                min: role.min,
                max: role.max,
            };
            match list.iter_mut().find(|series| series.role == role.role) {
                Some(series) => series.points.push(point),
                None => list.push(Series {
                    role: role.role.clone(),
                    points: vec![point],
                }),
            }
        }
    }
    for series in &mut list {
        series.points.sort_by(|a, b| a.width.total_cmp(&b.width));
    }
    list
}

/// The roles as `list` answers them: per view `{min, max}`, `growth`, `scaled`, `jump`.
#[must_use]
pub fn json(views: &[&ViewFonts], settings: &Settings) -> Vec<Value> {
    series(views)
        .iter()
        .map(|series| {
            let growth = series.growth();
            let points: Vec<Value> = series
                .points
                .iter()
                .map(|point| {
                    json!({ "view": point.view.view.id, "width": point.width, "min": point.min, "max": point.max })
                })
                .collect();
            json!({
                "role": series.role,
                "views": points,
                "growth": growth.map(|g| (g * 100.0).round() / 100.0),
                "scaled": growth.map(|g| g >= settings.fluid_min_ratio),
                "jump": series.jump(),
            })
        })
        .collect()
}

/// The `fluid-not-scaled` findings of the views showing one page, reported in view `lead`.
#[must_use]
pub fn findings(
    enabled: &[String],
    lead: ViewId,
    views: &[&ViewFonts],
    settings: &Settings,
) -> Vec<Finding> {
    let Some(first) = views.iter().find(|view| view.view.id == lead) else {
        return Vec::new();
    };
    let url = if first.fonts.url.is_empty() {
        first.view.url.as_str()
    } else {
        first.fonts.url.as_str()
    };
    let mut sink = Sink::new(enabled, lead, url);
    for series in series(views) {
        if !FLUID_ROLES.contains(&series.role.as_str()) {
            continue;
        }
        let Some(growth) = series.growth() else {
            continue;
        };
        if growth >= settings.fluid_min_ratio {
            continue;
        }
        let (Some(narrow), Some(wide)) = (series.points.first(), series.points.last()) else {
            continue;
        };
        let role = wide
            .view
            .fonts
            .roles
            .iter()
            .find(|role| role.role == series.role);
        let Some(role) = role else {
            continue;
        };
        sink.emit(
            rules::FLUID_NOT_SCALED,
            rules::FLUID_NOT_SCALED,
            &At::Element(&role.first),
            &[
                ("role", series.role.clone()),
                ("narrowSize", shown(narrow.max)),
                ("narrow", shown(narrow.width)),
                ("wideSize", shown(wide.max)),
                ("wide", shown(wide.width)),
                ("ratio", shown(settings.fluid_min_ratio)),
            ],
        );
    }
    sink.findings
}

#[cfg(test)]
#[path = "fluid_tests.rs"]
mod tests;
