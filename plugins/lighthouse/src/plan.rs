//! Which breakpoints a run measures and how: one target per breakpoint (the first view showing
//! it), with the view's size and device pixel ratio, mobile from the breakpoint's `mobile`, else
//! its `touch`, else a width below [`MOBILE_BELOW`], and the breakpoint's user agent. Firefox and
//! WebKit views are skipped (Lighthouse needs Chromium), and so is a view without an `http(s)`
//! page; each skip names its reason.

use rb_plugin_api::{ViewId, ViewInfo};
use serde_json::{Value, json};

use crate::project::Project;
use crate::rules::CATEGORIES;

/// A breakpoint narrower than this is measured as mobile unless it says otherwise.
pub const MOBILE_BELOW: u32 = 768;
const MAX_SIZE: u32 = 10_000;

/// One Lighthouse run of a run.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub view: ViewId,
    /// The breakpoint's id, or `view-<id>` for a view of a free size.
    pub breakpoint: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub dpr: f64,
    pub mobile: bool,
    pub user_agent: Option<String>,
}

/// A breakpoint the run leaves out, and why: `engine`, `url` or `not_found`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub breakpoint: String,
    pub view: Option<ViewId>,
    pub engine: Option<String>,
    pub reason: &'static str,
}

/// What a call narrows the run to.
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection<'a> {
    /// Only these breakpoints.
    pub only: Option<&'a [String]>,
    /// Only this view.
    pub view: Option<ViewId>,
    /// This page instead of each view's.
    pub url: Option<&'a str>,
}

/// Whether `url` is a page Lighthouse can load.
#[must_use]
pub fn is_web_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn name_of(view: &ViewInfo) -> String {
    if view.breakpoint.is_empty() {
        format!("view-{}", view.id)
    } else {
        view.breakpoint.clone()
    }
}

/// The targets and the skipped breakpoints of `views`.
#[must_use]
pub fn targets(
    views: &[ViewInfo],
    project: &Project,
    selection: &Selection<'_>,
) -> (Vec<Target>, Vec<Skipped>) {
    let mut targets: Vec<Target> = Vec::new();
    let mut skipped: Vec<Skipped> = Vec::new();
    for view in views {
        let breakpoint = name_of(view);
        let seen = targets.iter().any(|target| target.breakpoint == breakpoint)
            || skipped.iter().any(|skip| skip.breakpoint == breakpoint);
        let wanted = selection.view.is_none_or(|id| id == view.id)
            && selection.only.is_none_or(|only| only.contains(&breakpoint));
        if seen || !wanted {
            continue;
        }
        let skip = |reason| Skipped {
            breakpoint: breakpoint.clone(),
            view: Some(view.id),
            engine: Some(view.engine.clone()),
            reason,
        };
        if view.engine != "chromium" {
            skipped.push(skip("engine"));
            continue;
        }
        let url = selection.url.unwrap_or(&view.url);
        if !is_web_url(url) {
            skipped.push(skip("url"));
            continue;
        }
        let extras = project
            .breakpoints
            .get(&view.breakpoint)
            .cloned()
            .unwrap_or_default();
        targets.push(Target {
            view: view.id,
            url: url.trim().to_owned(),
            width: view.width.clamp(1, MAX_SIZE),
            height: view.height.clamp(1, MAX_SIZE),
            dpr: f64::from(view.dpr).clamp(0.1, 10.0),
            mobile: extras
                .mobile
                .or(extras.touch)
                .unwrap_or(view.width < MOBILE_BELOW),
            user_agent: extras.user_agent,
            breakpoint,
        });
    }
    for name in selection.only.unwrap_or_default() {
        let known = targets.iter().any(|target| &target.breakpoint == name)
            || skipped.iter().any(|skip| &skip.breakpoint == name);
        if !known && selection.view.is_none() {
            skipped.push(Skipped {
                breakpoint: name.clone(),
                view: None,
                engine: None,
                reason: "not_found",
            });
        }
    }
    (targets, skipped)
}

/// The categories of a run: the call's, else the project's, else all four - in Lighthouse's
/// order.
#[must_use]
pub fn categories(asked: Option<&[String]>, project: &Project) -> Vec<String> {
    let chosen = match asked {
        Some(list) if !list.is_empty() => list,
        _ => project.categories.as_slice(),
    };
    CATEGORIES
        .iter()
        .filter(|category| chosen.is_empty() || chosen.iter().any(|c| c == *category))
        .map(|category| (*category).to_owned())
        .collect()
}

impl Target {
    /// The settings `lighthouse.start` takes for this target.
    #[must_use]
    pub fn settings(&self, throttling: &str, categories: &[String]) -> Value {
        let mut settings = json!({
            "formFactor": if self.mobile { "mobile" } else { "desktop" },
            "screenEmulation": {
                "width": self.width,
                "height": self.height,
                "deviceScaleFactor": self.dpr,
                "mobile": self.mobile,
            },
            "throttlingMethod": throttling,
            "onlyCategories": categories,
        });
        if let Some(agent) = &self.user_agent {
            settings["emulatedUserAgent"] = json!(agent);
        }
        settings
    }

    /// The breakpoint as a file name: anything but `[A-Za-z0-9._-]` becomes `-`.
    #[must_use]
    pub fn file_name(&self) -> String {
        let name: String = self
            .breakpoint
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        match name.trim_start_matches('.') {
            "" => format!("view-{}", self.view),
            trimmed => trimmed.to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
