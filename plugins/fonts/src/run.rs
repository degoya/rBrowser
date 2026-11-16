//! Collecting the fonts of the views a call targets - the page part through `evaluate`, the
//! rendered fonts of samples over CDP - and the check run of `checks_run`.

use rb_plugin_api::{
    CheckContext, Failure, FailureKind, Finding, PluginHost, ViewId, ViewInfo, codes,
};
use serde_json::Value;

use crate::checks;
use crate::fluid;
use crate::page::{self, Located, PageFonts};
use crate::rendered::{self, RenderedFont, Verdict};
use crate::rules::{self, FLUID_NOT_SCALED, RENDERED_RULES};
use crate::settings::Settings;

/// The most elements whose rendered fonts are read per view (two CDP calls each).
pub const MAX_RENDERED: usize = 40;

/// One element's rendered fonts against its declared family.
#[derive(Clone, Debug, PartialEq)]
pub struct SampleResult {
    /// The declared first family.
    pub family: String,
    pub stack: String,
    pub weight: u64,
    pub style: String,
    pub at: Located,
    pub text: String,
    pub non_ascii: String,
    pub fonts: Vec<RenderedFont>,
    pub verdict: Verdict,
}

/// Whether the rendered fonts were read in a view.
#[derive(Clone, Debug, PartialEq)]
pub enum Rendered {
    Read(Vec<SampleResult>),
    /// No CDP: a Firefox or WebKit view.
    Unsupported,
    /// Not asked for.
    Skipped,
}

impl Rendered {
    #[must_use]
    pub fn results(&self) -> Option<&[SampleResult]> {
        match self {
            Self::Read(results) => Some(results),
            Self::Unsupported | Self::Skipped => None,
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Read(_) => "read",
            Self::Unsupported => "unsupported",
            Self::Skipped => "skipped",
        }
    }
}

/// One view's fonts.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewFonts {
    pub view: ViewInfo,
    pub fonts: PageFonts,
    pub rendered: Rendered,
}

impl ViewFonts {
    /// The view's width in CSS px: the page's, else the grid's.
    #[must_use]
    pub fn width(&self) -> f64 {
        if self.fonts.viewport.width > 0.0 {
            self.fonts.viewport.width
        } else {
            f64::from(self.view.width)
        }
    }
}

/// The fonts of the views, and why the others gave none.
#[derive(Debug, Default)]
pub struct Collected {
    pub views: Vec<ViewFonts>,
    /// `(view, failure code)`: a Firefox or WebKit view (`not_available`), a view that went away.
    pub failures: Vec<(ViewInfo, String)>,
}

/// The views `target` names: `"all"` or none for every view, a view id for that one.
///
/// # Errors
///
/// `plugins.error.not_found` for a view the window does not have, `plugins.error.invalid` for
/// any other target.
pub fn targets(host: &mut dyn PluginHost, target: &Value) -> Result<Vec<ViewInfo>, Failure> {
    let views = host.views()?;
    match target {
        Value::Null => Ok(views),
        Value::String(all) if all == "all" => Ok(views),
        Value::Number(number) => {
            let id = number
                .as_u64()
                .and_then(|id| ViewId::try_from(id).ok())
                .ok_or_else(|| Failure::coded(FailureKind::BadData, codes::INVALID))?;
            views
                .into_iter()
                .find(|view| view.id == id)
                .map(|view| vec![view])
                .ok_or_else(|| {
                    Failure::coded(FailureKind::NotFound, codes::NOT_FOUND).with_param("view", id)
                })
        }
        _ => {
            Err(Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", "target"))
        }
    }
}

/// The samples whose rendered fonts are read: per usage (most text first) its first sample and
/// every one with characters beyond ASCII, generic families left out; at most [`MAX_RENDERED`].
fn samples(fonts: &PageFonts) -> Vec<(usize, usize)> {
    let mut order: Vec<usize> = (0..fonts.usages.len()).collect();
    order.sort_by_key(|index| std::cmp::Reverse(fonts.usages[*index].count));
    let mut chosen = Vec::new();
    for index in order {
        let usage = &fonts.usages[index];
        if rendered::generic(&usage.family) || usage.family.is_empty() {
            continue;
        }
        for (at, sample) in usage.samples.iter().enumerate() {
            if (at == 0 || !sample.non_ascii.is_empty()) && !sample.at.selector.is_empty() {
                chosen.push((index, at));
            }
        }
    }
    chosen.truncate(MAX_RENDERED);
    chosen
}

fn read_rendered(host: &mut dyn PluginHost, view: ViewId, fonts: &PageFonts) -> Rendered {
    let chosen = samples(fonts);
    let selectors: Vec<&str> = chosen
        .iter()
        .map(|(usage, sample)| fonts.usages[*usage].samples[*sample].at.selector.as_str())
        .collect();
    let Ok(read) = rendered::read(host, view, &selectors) else {
        return Rendered::Unsupported;
    };
    let results = chosen
        .iter()
        .zip(read)
        .map(|((usage, sample), drawn)| {
            let usage = &fonts.usages[*usage];
            let sample = &usage.samples[*sample];
            let drawn = drawn.unwrap_or_default();
            SampleResult {
                family: usage.family.clone(),
                stack: usage.stack.clone(),
                weight: usage.weight,
                style: usage.style.clone(),
                at: sample.at.clone(),
                text: sample.text.clone(),
                non_ascii: sample.non_ascii.clone(),
                verdict: rendered::judge(&usage.stack, &fonts.faces, &drawn),
                fonts: drawn,
            }
        })
        .collect();
    Rendered::Read(results)
}

/// `view`'s fonts, the rendered ones when `with_rendered`.
///
/// # Errors
///
/// The host's failure to evaluate (no such view, not Chromium) or a malformed answer.
pub fn collect_view(
    host: &mut dyn PluginHost,
    view: &ViewInfo,
    settings: &Settings,
    with_rendered: bool,
) -> Result<ViewFonts, Failure> {
    let fonts = page::collect(host, view.id, settings)?;
    let rendered = if with_rendered {
        read_rendered(host, view.id, &fonts)
    } else {
        Rendered::Skipped
    };
    Ok(ViewFonts {
        view: view.clone(),
        fonts,
        rendered,
    })
}

/// The fonts of `views`.
pub fn collect(
    host: &mut dyn PluginHost,
    views: &[ViewInfo],
    settings: &Settings,
    with_rendered: bool,
) -> Collected {
    let mut collected = Collected::default();
    let total = u32::try_from(views.len()).unwrap_or(u32::MAX);
    for (done, view) in views.iter().enumerate() {
        match collect_view(host, view, settings, with_rendered) {
            Ok(fonts) => collected.views.push(fonts),
            Err(failure) => collected.failures.push((
                view.clone(),
                failure.code().unwrap_or(codes::HOST_FAILED).to_owned(),
            )),
        }
        host.report_progress(u32::try_from(done + 1).unwrap_or(u32::MAX), total);
    }
    collected
}

/// The breakpoint findings of every page `leads` lead (the first view showing its URL): the views
/// showing the same URL, read once (`cache`), compared.
fn fluid_findings(
    host: &mut dyn PluginHost,
    enabled: &[String],
    settings: &Settings,
    all: &[ViewInfo],
    leads: &[ViewId],
    cache: &mut Vec<ViewFonts>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    for lead in leads {
        let Some(url) = all
            .iter()
            .find(|view| view.id == *lead)
            .map(|view| view.url.clone())
        else {
            continue;
        };
        let same: Vec<&ViewInfo> = all
            .iter()
            .filter(|view| view.engine == "chromium" && view.url == url)
            .collect();
        if same.first().is_none_or(|first| first.id != *lead) {
            continue;
        }
        for view in &same {
            if !cache.iter().any(|known| known.view.id == view.id)
                && let Ok(fonts) = collect_view(host, view, settings, false)
            {
                cache.push(fonts);
            }
        }
        let group: Vec<&ViewFonts> = same
            .iter()
            .filter_map(|view| cache.iter().find(|known| known.view.id == view.id))
            .collect();
        findings.extend(fluid::findings(enabled, *lead, &group, settings));
    }
    findings
}

/// The check run of one view (or of every view when the run names none). The breakpoint rule
/// runs in the first view of each URL, over every view showing it.
///
/// # Errors
///
/// The host's failure for the one view the run names; a Firefox or WebKit view named alone
/// answers `Failure::engine_unserved` (the runner shows it skipped), left out of a run over all.
pub fn check(host: &mut dyn PluginHost, ctx: &CheckContext) -> Result<Vec<Finding>, Failure> {
    let settings = Settings::read(host);
    let all = host.views()?;
    let chosen: Vec<ViewInfo> = match ctx.view {
        Some(id) => {
            // Gone: not found; another engine: the runner shows it skipped, not "nothing
            // found" (RB-080-98).
            vec![ViewInfo::chromium_of(&all, id, "evaluate")?]
        }
        None => all.clone(),
    };
    let with_rendered = RENDERED_RULES.iter().any(|id| rules::wants(&ctx.rules, id));
    let mut cache: Vec<ViewFonts> = Vec::new();
    let mut findings = Vec::new();
    for view in &chosen {
        let fonts = match collect_view(host, view, &settings, with_rendered) {
            Ok(fonts) => fonts,
            Err(failure) if failure.code() == Some(codes::NOT_AVAILABLE) => continue,
            Err(failure) if ctx.view.is_some() => return Err(failure),
            Err(_) => continue,
        };
        findings.extend(checks::view(&ctx.rules, &fonts, &settings));
        cache.push(fonts);
    }
    if rules::wants(&ctx.rules, FLUID_NOT_SCALED) {
        let leads: Vec<ViewId> = chosen.iter().map(|view| view.id).collect();
        findings.extend(fluid_findings(
            host, &ctx.rules, &settings, &all, &leads, &mut cache,
        ));
    }
    Ok(findings)
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
