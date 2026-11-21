//! The words of the Lighthouse bar under every view (RB-090-73, `ui.viewFooter.lighthouse`):
//! the four category scores of the newest Lighthouse run of the view's page and breakpoint -
//! Performance, Accessibility, Best Practices, SEO - each a gauge with Lighthouse's colour and
//! the shape of its rating, the run's time and every score in words in the tooltip. Pure: a
//! run's summary (`run.json` of the Lighthouse plugin, `plugins/lighthouse/src/run.rs`) in,
//! words and colours out; the disk is [`super::lighthouse_runs`], the strip
//! `cef_app/cell_footer_rows.rs`.
//!
//! **Gauges as text.** A native strip's button draws text in one colour; a drawn arc would be
//! a bitmap per score made at every write. Each gauge is therefore its own button: the shape of
//! its rating (circle 90-100, square 50-89, triangle 0-49 - never colour alone, as in the vitals
//! bar) and the score, in Lighthouse's colour for the rating on the theme's ground, then the
//! category's name - in full, short (`Perf`, `A11y`, `BP`, `SEO`) or left out, whichever fits the
//! strip ([`form_for`]).
//!
//! **Which run.** The newest run whose breakpoint is the view's (a free size is `view-<id>`, as
//! the plugin names it) and whose measured address - asked for or after redirects - is the
//! page the view shows, both without the fragment and normalised; a run of another page is not
//! shown. Without such a run the view has no Lighthouse bar.

use rb_engine::ViewId;
use rb_i18n::{t, t_with};
use rb_ipc::Rating;
use serde_json::Value;

use super::model::contrast;
use super::vitals::shape_of;
use super::wrap::text_width;

/// The plugin whose runs the bar shows.
pub const PLUGIN: &str = "lighthouse";
/// The plugin's command a click runs: the run's report of the view's breakpoint.
pub const OPEN_REPORT: &str = "plugin.lighthouse.open_report";
/// The most breakpoints read of one run (the plugin measures at most 32).
const MAX_BREAKPOINTS: usize = 64;
/// The longest breakpoint name read.
const MAX_NAME: usize = 64;

/// Lighthouse's colours by rating: on a light ground (its report's light theme) and on a dark
/// one (its dark theme); [`gauge_color`] takes the one that reads on the ground.
const COLORS: [(Rating, u32, u32); 3] = [
    (Rating::Good, 0x0000_8800, 0x000c_ce6b),
    (Rating::NeedsImprovement, 0x00c3_3300, 0x00ff_a400),
    (Rating::Poor, 0x00cc_0000, 0x00ff_4e42),
];

/// A Lighthouse category the bar shows, in Lighthouse's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Performance,
    Accessibility,
    BestPractices,
    Seo,
}

impl Category {
    /// Every category, in the bar's order.
    pub const ALL: [Self; 4] = [
        Self::Performance,
        Self::Accessibility,
        Self::BestPractices,
        Self::Seo,
    ];

    /// Lighthouse's id, the key of the summary's `scores`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Performance => "performance",
            Self::Accessibility => "accessibility",
            Self::BestPractices => "best-practices",
            Self::Seo => "seo",
        }
    }

    fn name(self) -> String {
        t(&format!("footer.lighthouse.category.{}", self.id()))
    }

    fn short(self) -> String {
        t(&format!("footer.lighthouse.short.{}", self.id()))
    }
}

/// One gauge: a category and its score 0 to 100, `None` when Lighthouse gave none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gauge {
    pub category: Category,
    pub score: Option<u8>,
}

/// One breakpoint of one run on disk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measured {
    /// The run id, `YYYYMMDD-HHMMSS` in UTC: newer sorts later.
    pub run: String,
    /// When the run started, seconds after the Unix epoch.
    pub started_at: Option<u64>,
    /// The breakpoint as the plugin names it.
    pub breakpoint: String,
    /// The addresses measured, normalised: asked for and after redirects.
    pub documents: Vec<String>,
    /// The categories the run scored, in [`Category::ALL`]'s order.
    pub gauges: Vec<Gauge>,
}

/// The Lighthouse bar of one view: its gauges, the tooltip, and the run and breakpoint whose
/// report a click opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LighthouseBar {
    pub gauges: Vec<Gauge>,
    pub tooltip: String,
    pub run: String,
    pub breakpoint: String,
}

/// How much of a gauge's category name the strip has room for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Form {
    /// `● 72 Performance`.
    #[default]
    Named,
    /// `● 72 Perf`.
    Short,
    /// `● 72`.
    Bare,
}

/// Lighthouse's rating of `score`: 0-49 poor, 50-89 needs improvement, 90-100 good.
pub fn rating(score: u8) -> Rating {
    match score {
        90.. => Rating::Good,
        50..=89 => Rating::NeedsImprovement,
        _ => Rating::Poor,
    }
}

/// `url` as the bar compares pages: parsed (scheme and host in lower case, no default port, a
/// path of at least `/`) and without its fragment; `None` for no address.
pub fn normalize(url: &str) -> Option<String> {
    let mut parsed = url::Url::parse(url.trim()).ok()?;
    parsed.set_fragment(None);
    Some(parsed.into())
}

/// The breakpoint of `view` as a run names it: its breakpoint's id, `view-<id>` for a free size.
pub fn breakpoint_name(view: ViewId, breakpoint: Option<&str>) -> String {
    match breakpoint.filter(|id| !id.is_empty()) {
        Some(id) => id.to_owned(),
        None => format!("view-{view}"),
    }
}

/// Whether `report` (as `run.json` writes it) is an HTML file of run `run`'s own folder, as the
/// plugin's history requires before it offers a report.
fn in_run(report: &str, run: &str) -> bool {
    let prefix = format!(".rbrowser/out/{PLUGIN}/{run}/");
    report.strip_prefix(&prefix).is_some_and(|file| {
        !file.is_empty()
            && !file.starts_with('.')
            && file.ends_with(".html")
            && file
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    })
}

/// The scores of one breakpoint, in the bar's order; a category not in `scores` is left out, a
/// score past 100 or of another type drops the category.
fn gauges(scores: &Value) -> Vec<Gauge> {
    let Some(scores) = scores.as_object() else {
        return Vec::new();
    };
    Category::ALL
        .iter()
        .filter_map(|category| {
            let score = match scores.get(category.id())? {
                Value::Null => None,
                value => Some(u8::try_from(value.as_u64()?).ok().filter(|s| *s <= 100)?),
            };
            Some(Gauge {
                category: *category,
                score,
            })
        })
        .collect()
}

/// The breakpoints of run `id` from its summary `bytes`: each with a report in the run's own
/// folder and at least one category; nothing from a summary that does not read.
pub fn parse_run(id: &str, bytes: &[u8]) -> Vec<Measured> {
    let Ok(summary) = serde_json::from_slice::<Value>(bytes) else {
        return Vec::new();
    };
    let started_at = summary["startedAt"].as_u64();
    let entries = summary["breakpoints"]
        .as_array()
        .map_or(&[][..], Vec::as_slice);
    entries
        .iter()
        .take(MAX_BREAKPOINTS)
        .filter_map(|entry| {
            let breakpoint = entry["breakpoint"]
                .as_str()
                .filter(|name| !name.is_empty() && name.len() <= MAX_NAME)?;
            if !entry["report"]
                .as_str()
                .is_some_and(|report| in_run(report, id))
            {
                return None;
            }
            let gauges = gauges(&entry["scores"]);
            if gauges.is_empty() {
                return None;
            }
            let mut documents: Vec<String> = ["url", "finalUrl"]
                .iter()
                .filter_map(|key| entry[*key].as_str().and_then(normalize))
                .collect();
            documents.dedup();
            Some(Measured {
                run: id.to_owned(),
                started_at,
                breakpoint: breakpoint.to_owned(),
                documents,
                gauges,
            })
        })
        .collect()
}

/// The newest of `runs` for the page `document` and the breakpoint `breakpoint`.
pub fn latest<'a>(runs: &'a [Measured], document: &str, breakpoint: &str) -> Option<&'a Measured> {
    let document = normalize(document)?;
    runs.iter()
        .filter(|run| run.breakpoint == breakpoint && run.documents.contains(&document))
        .max_by(|a, b| a.run.cmp(&b.run))
}

/// `secs` after the Unix epoch in the zone `offset` seconds east of UTC: `2026-11-17 14:32`.
pub fn run_time(secs: u64, offset: i32) -> String {
    let zone = time::UtcOffset::from_whole_seconds(offset).unwrap_or(time::UtcOffset::UTC);
    i64::try_from(secs)
        .ok()
        .and_then(|secs| time::OffsetDateTime::from_unix_timestamp(secs).ok())
        .and_then(|instant| instant.checked_to_offset(zone))
        .map(|time| {
            format!(
                "{:04}-{:02}-{:02} {:02}:{:02}",
                time.year(),
                u8::from(time.month()),
                time.day(),
                time.hour(),
                time.minute()
            )
        })
        .unwrap_or_default()
}

fn rating_id(rating: Rating) -> &'static str {
    match rating {
        Rating::Good => "good",
        Rating::NeedsImprovement => "needs-improvement",
        Rating::Poor => "poor",
    }
}

/// One gauge in words: `Performance: 72 (needs improvement)`, or that it has no score.
pub fn spoken(gauge: &Gauge) -> String {
    let category = gauge.category.name();
    match gauge.score {
        Some(score) => t_with(
            "footer.lighthouse.score",
            &[
                ("category", &category),
                ("score", &score.to_string()),
                (
                    "rating",
                    &t(&format!("shell.vitals.rating.{}", rating_id(rating(score)))),
                ),
            ],
        ),
        None => t_with("footer.lighthouse.noScore", &[("category", &category)]),
    }
}

/// The bar of `measured`: its gauges and a tooltip with the run's time in the zone `offset`
/// seconds east of UTC (the local one), every score in words and what a click does.
pub fn bar(measured: &Measured, offset: i32) -> LighthouseBar {
    let mut lines = Vec::with_capacity(measured.gauges.len() + 2);
    lines.push(match measured.started_at {
        Some(secs) => t_with(
            "footer.lighthouse.run",
            &[("time", &run_time(secs, offset))],
        ),
        None => t("footer.lighthouse.title"),
    });
    lines.extend(measured.gauges.iter().map(spoken));
    lines.push(t("footer.lighthouse.open"));
    LighthouseBar {
        gauges: measured.gauges.clone(),
        tooltip: lines.join("\n"),
        run: measured.run.clone(),
        breakpoint: measured.breakpoint.clone(),
    }
}

/// What the gauge's button says in `form`.
pub fn gauge_text(gauge: &Gauge, form: Form) -> String {
    let name = match form {
        Form::Named => Some(gauge.category.name()),
        Form::Short => Some(gauge.category.short()),
        Form::Bare => None,
    };
    let value = match gauge.score {
        Some(score) => format!("{} {score}", shape_of(rating(score))),
        None => t("footer.lighthouse.none"),
    };
    match name {
        Some(name) => t_with(
            "footer.lighthouse.gauge",
            &[("value", &value), ("category", &name)],
        ),
        None => value,
    }
}

/// The fullest form whose buttons together fit a strip `available` DIP wide (`None`: not laid
/// out yet, taken as wide enough); [`Form::Bare`] when none does.
pub fn form_for(gauges: &[Gauge], available: Option<u32>) -> Form {
    let Some(available) = available.filter(|width| *width > 0) else {
        return Form::Named;
    };
    [Form::Named, Form::Short]
        .into_iter()
        .find(|form| {
            let width: u32 = gauges
                .iter()
                .map(|gauge| text_width(&gauge_text(gauge, *form)))
                .fold(0, u32::saturating_add);
            width <= available
        })
        .unwrap_or(Form::Bare)
}

/// Lighthouse's colour (`0xrrggbb`) of `score` on `ground`: of its light and its dark theme's,
/// the one that stands off `ground` more; `None` without a score - the theme's text then.
pub fn gauge_color(score: Option<u8>, ground: u32) -> Option<u32> {
    let rated = rating(score?);
    let (_, light, dark) = COLORS.iter().find(|(rating, _, _)| *rating == rated)?;
    Some(if contrast(*light, ground) >= contrast(*dark, ground) {
        *light
    } else {
        *dark
    })
}

#[cfg(test)]
#[path = "lighthouse_tests.rs"]
mod tests;
