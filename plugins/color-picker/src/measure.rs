//! Characters per line: the command `measure_text` (`Ctrl+Shift+Space D M`), the MCP tool
//! `color_measure_text` and the element a person chose in a view ([`crate::page`]).
//!
//! `src/measure.js` runs in each view's plugin world (`evaluate`): for every rendered element
//! the selector matches it splits the element's text into lines by `Range.getClientRects()` and
//! answers each line's characters and box. Here the lines are rated: the average leaves out an
//! element's last line (it is as short as the text happens to end), the maximum takes every
//! line; an average below `optimalMin` is `short`, above `optimalMax` `long`, else `optimal`;
//! lines above `warnAbove` (WCAG 1.4.8) are posted to their view's page script to be marked.

use rb_plugin_api::{Failure, FailureKind, PluginHost, ViewInfo};
use serde_json::{Value, json};

use crate::settings::{MAX_SELECTOR, Settings};
use crate::targets::{self, Default};
use crate::{bad_data, failure, invalid, layer_name, panel};

/// The collector bundle: a function taking `{ selector, maxElements, maxLines }`.
pub const COLLECTOR: &str = include_str!("measure.js");
/// The most elements one view's collector measures.
pub const MAX_ELEMENTS: u64 = 500;
/// The most lines one view's collector answers.
pub const MAX_LINES: u64 = 5_000;
/// The most elements answered per view.
pub const MAX_ITEMS: usize = 50;
/// The most marks posted to one view.
pub const MAX_MARKS: usize = 200;
/// The most views that get marks (posts are at most 16, one is the panel's).
pub const MAX_MARKED_VIEWS: usize = 15;

/// Lines of one element or one view, rated.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stats {
    pub lines: usize,
    /// Characters per line, the elements' last lines left out; one decimal.
    pub average: f64,
    pub max: u64,
    /// Lines above `warnAbove`.
    pub over: usize,
}

impl Stats {
    /// The stats of elements given as their lines' character counts.
    #[must_use]
    pub fn of(elements: &[Vec<u64>], warn_above: u64) -> Self {
        let mut body: Vec<u64> = Vec::new();
        let mut stats = Self::default();
        for lines in elements {
            stats.lines += lines.len();
            stats.max = stats.max.max(lines.iter().copied().max().unwrap_or(0));
            stats.over += lines.iter().filter(|chars| **chars > warn_above).count();
            let kept = if lines.len() > 1 {
                &lines[..lines.len() - 1]
            } else {
                &lines[..]
            };
            body.extend_from_slice(kept);
        }
        if !body.is_empty() {
            let sum: u64 = body.iter().sum();
            let average = sum as f64 / body.len() as f64;
            stats.average = (average * 10.0).round() / 10.0;
        }
        stats
    }

    /// `none`, `short`, `optimal` or `long` against the settings' range.
    #[must_use]
    pub fn rating(&self, settings: &Settings) -> &'static str {
        if self.lines == 0 {
            "none"
        } else if self.average < f64::from(settings.optimal_min) {
            "short"
        } else if self.average > f64::from(settings.optimal_max) {
            "long"
        } else {
            "optimal"
        }
    }
}

fn chars(lines: &Value) -> Vec<u64> {
    lines
        .as_array()
        .map(|lines| {
            lines
                .iter()
                .map(|line| line["chars"].as_u64().unwrap_or(0))
                .collect()
        })
        .unwrap_or_default()
}

/// One view's measurement as the answer gives it, and the marks for its page script.
#[must_use]
pub fn rate(view: &ViewInfo, page: &Value, settings: &Settings) -> (Value, Vec<Value>) {
    let warn = u64::from(settings.warn_above);
    let elements = page["elements"].as_array().cloned().unwrap_or_default();
    let counts: Vec<Vec<u64>> = elements.iter().map(|e| chars(&e["lines"])).collect();
    let total = Stats::of(&counts, warn);
    let items: Vec<Value> = elements
        .iter()
        .zip(&counts)
        .take(MAX_ITEMS)
        .map(|(element, lines)| {
            let stats = Stats::of(std::slice::from_ref(lines), warn);
            json!({
                "path": element["path"].as_str().unwrap_or_default(),
                "lines": stats.lines,
                "average": stats.average,
                "max": stats.max,
                "over": stats.over,
                "rating": stats.rating(settings),
            })
        })
        .collect();
    let marks: Vec<Value> = elements
        .iter()
        .flat_map(|element| element["lines"].as_array().cloned().unwrap_or_default())
        .filter(|line| line["chars"].as_u64().unwrap_or(0) > warn)
        .take(MAX_MARKS)
        .map(|line| {
            json!({
                "x": line["x"].as_f64().unwrap_or(0.0),
                "y": line["y"].as_f64().unwrap_or(0.0),
                "width": line["width"].as_f64().unwrap_or(0.0),
                "height": line["height"].as_f64().unwrap_or(0.0),
                "chars": line["chars"].as_u64().unwrap_or(0),
            })
        })
        .collect();
    let answer = json!({
        "view": view.id,
        "label": view.label,
        "breakpoint": view.breakpoint,
        "width": view.width,
        "elements": elements.len(),
        "lines": total.lines,
        "average": total.average,
        "max": total.max,
        "over": total.over,
        "rating": total.rating(settings),
        "items": items,
        "truncated": page["truncated"].as_bool().unwrap_or(false),
    });
    (answer, marks)
}

/// The script `evaluate` runs: the collector, called with `selector`.
#[must_use]
pub fn script(selector: &str) -> String {
    let options =
        json!({ "selector": selector, "maxElements": MAX_ELEMENTS, "maxLines": MAX_LINES });
    format!("({})({options})", COLLECTOR.trim())
}

/// The selector of `args`, else the setting's.
fn selector(args: &Value, settings: &Settings) -> Result<String, Failure> {
    match &args["selector"] {
        Value::Null => Ok(settings.selector.clone()),
        Value::String(text) => {
            let text = text.trim();
            if text.is_empty() || text.chars().count() > MAX_SELECTOR {
                return Err(invalid("selector"));
            }
            Ok(text.to_owned())
        }
        _ => Err(invalid("selector")),
    }
}

/// Measures `selector` in the views `args["target"]` names (every view without one): the answer
/// with the posts of its marks and of the panel's copy.
///
/// # Errors
///
/// `invalid` for bad arguments; the views' failure.
pub fn measure(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let settings = Settings::read(host);
    let selector = selector(args, &settings)?;
    let views = targets::resolve(host, args, Default::All)?;
    let script = script(&selector);
    let mut answers = Vec::new();
    let mut posts = Vec::new();
    for view in views {
        if !targets::supported(&view) {
            answers.push(targets::unsupported(&view));
            continue;
        }
        let read = host
            .evaluate(view.id, &script)
            .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|_| bad_data("measure")))
            .and_then(|page| {
                if page["invalidSelector"].as_bool() == Some(true) {
                    Err(failure(FailureKind::BadData, "bad_selector"))
                } else {
                    Ok(page)
                }
            });
        match read {
            Ok(page) => {
                let (answer, marks) = rate(&view, &page, &settings);
                if posts.len() < MAX_MARKED_VIEWS {
                    let message = json!({ "type": "lines", "warnAbove": settings.warn_above, "marks": marks });
                    posts.push(json!({ "target": view.id, "message": message }));
                }
                answers.push(answer);
            }
            Err(failure) => answers.push(targets::failed(&view, &failure)),
        }
    }
    let result = json!({
        "selector": selector,
        "optimal": { "min": settings.optimal_min, "max": settings.optimal_max },
        "warnAbove": settings.warn_above,
        "views": answers,
    });
    posts.push(json!({ "target": "panel", "message": { "type": "measured", "result": result } }));
    let mut answer = result;
    answer["$post"] = Value::Array(posts);
    Ok(answer)
}

/// `color_measure_text`: `clear` removes the marks, `choose` lets a person click the element to
/// measure in a view, else [`measure`].
///
/// # Errors
///
/// As [`measure`].
pub fn tool(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let flag = |key: &str| match &args[key] {
        Value::Null => Ok(false),
        Value::Bool(on) => Ok(*on),
        _ => Err(invalid(key)),
    };
    if flag("clear")? {
        return Ok(json!({
            "cleared": true,
            "$post": [{ "target": "all", "message": { "type": "clear" } }],
        }));
    }
    if flag("choose")? {
        return Ok(json!({
            "choosing": true,
            "$post": [{ "target": "all", "message": { "type": "choose", "label": layer_name() } }],
        }));
    }
    measure(host, args)
}

/// The command `measure_text`: measures with the setting's selector in every view and shows the
/// panel.
///
/// # Errors
///
/// As [`measure`].
pub fn command(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let mut answer = measure(host, args)?;
    answer["$open"] = panel()["$open"].clone();
    Ok(answer)
}

#[cfg(test)]
#[path = "measure_tests.rs"]
mod tests;
