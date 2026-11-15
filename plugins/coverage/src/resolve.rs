//! CDP's raw coverage turned into files: Chromium's rule usage names style sheets by an id only,
//! so each sheet's text (`CSS.getStyleSheetText`) is matched with the sheets the document lists -
//! a `<style>` element by its text, a linked sheet by its content (`Page.getResourceContent`) -
//! which also gives the sheets no rule of which was used their size. V8's precise coverage names
//! each script by URL, with its functions' block ranges.

use std::collections::{BTreeMap, BTreeSet};

use rb_plugin_api::{Failure, PluginHost, ViewId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::css_rules::units;
use crate::model::{FileRun, Function, Kind, MAX_FUNCTIONS, MAX_RULES};
use crate::page::{self, Sheets};
use crate::ranges::{Count, Segment, length, merge, used_segments};
use crate::settings::{Settings, origin};
use crate::store;

/// One entry of `CSS.takeCoverageDelta` / `CSS.stopRuleUsageTracking`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RuleUsage {
    pub style_sheet_id: String,
    pub start_offset: f64,
    pub end_offset: f64,
    #[serde(default)]
    pub used: bool,
}

/// One range of V8's coverage.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRange {
    pub start_offset: u32,
    pub end_offset: u32,
    #[serde(default)]
    pub count: u64,
}

/// One function of V8's coverage; its first range is the whole function.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FunctionCoverage {
    #[serde(default)]
    pub function_name: String,
    #[serde(default)]
    pub ranges: Vec<CoverageRange>,
}

/// One script of `Profiler.takePreciseCoverage`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptCoverage {
    pub script_id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub functions: Vec<FunctionCoverage>,
}

/// What one call has fetched already: linked sheets' content by URL, and the sources written.
#[derive(Debug, Default)]
pub struct Cache {
    content: BTreeMap<String, Option<String>>,
    pub written: BTreeSet<String>,
}

fn unit(value: f64) -> u32 {
    if value.is_finite() && value > 0.0 {
        // CDP's offsets are whole numbers in a JSON number.
        value.min(f64::from(u32::MAX)).round() as u32
    } else {
        0
    }
}

/// A URL without its fragment.
fn page_of(url: &str) -> &str {
    url.split_once('#').map_or(url, |(page, _)| page)
}

fn content(
    host: &mut dyn PluginHost,
    view: ViewId,
    frame: Option<&str>,
    href: &str,
    cache: &mut Cache,
) -> Option<String> {
    if let Some(known) = cache.content.get(href) {
        return known.clone();
    }
    let text = frame.and_then(|frame| {
        let params = json!({ "frameId": frame, "url": href }).to_string();
        let answer = host
            .cdp_send(view, "Page.getResourceContent", &params)
            .ok()?;
        let value: Value = serde_json::from_str(&answer).ok()?;
        // A style sheet is text; a base64 answer is something else.
        (!value["base64Encoded"].as_bool().unwrap_or(false))
            .then(|| value["content"].as_str().map(str::to_owned))
            .flatten()
    });
    cache.content.insert(href.to_owned(), text.clone());
    text
}

fn main_frame(host: &mut dyn PluginHost, view: ViewId) -> Option<String> {
    let answer = host.cdp_send(view, "Page.getFrameTree", "{}").ok()?;
    let value: Value = serde_json::from_str(&answer).ok()?;
    value["frameTree"]["frame"]["id"]
        .as_str()
        .map(str::to_owned)
}

/// A sheet the document lists, with its text and the used rules matched to it.
struct Listed {
    href: Option<String>,
    text: Option<String>,
    rules: Vec<Segment>,
    claimed: bool,
}

fn css_file(url: String, inline: bool, text: &str, rules: Vec<Segment>) -> FileRun {
    let total = units(text);
    let segments = merge(rules.clone());
    let mut rules = rules;
    rules.sort_unstable();
    rules.dedup();
    rules.truncate(MAX_RULES);
    FileRun {
        url,
        kind: Kind::Css,
        inline,
        total,
        used: length(&segments).min(total),
        segments,
        rules,
        functions: Vec::new(),
        source: None,
    }
}

/// The CSS files of `view` from the used rules `usage`.
///
/// # Errors
///
/// The page could not be read (`evaluate`).
pub fn css(
    host: &mut dyn PluginHost,
    view: ViewId,
    usage: &[RuleUsage],
    settings: &Settings,
    cache: &mut Cache,
) -> Result<(String, Vec<FileRun>), Failure> {
    let Sheets {
        url: page_url,
        sheets,
    } = page::sheets(host, view)?;
    let mut used: BTreeMap<&str, Vec<Segment>> = BTreeMap::new();
    for rule in usage.iter().filter(|rule| rule.used) {
        used.entry(rule.style_sheet_id.as_str())
            .or_default()
            .push((unit(rule.start_offset), unit(rule.end_offset)));
    }
    let frame = main_frame(host, view);
    // The document's sheets with their text: inline as listed, linked as fetched.
    let mut listed: Vec<Listed> = Vec::new();
    for sheet in sheets {
        let text = match (&sheet.href, sheet.text) {
            (Some(href), _) if origin(href).is_some() => {
                content(host, view, frame.as_deref(), href, cache)
            }
            (Some(_), _) => None,
            (None, text) => text,
        };
        listed.push(Listed {
            href: sheet.href,
            text,
            rules: Vec::new(),
            claimed: false,
        });
    }
    let mut unmatched = Vec::new();
    for (id, rules) in used {
        let params = json!({ "styleSheetId": id }).to_string();
        let Some(text) = host
            .cdp_send(view, "CSS.getStyleSheetText", &params)
            .ok()
            .and_then(|answer| serde_json::from_str::<Value>(&answer).ok())
            .and_then(|value| value["text"].as_str().map(str::to_owned))
        else {
            continue;
        };
        match listed
            .iter_mut()
            .find(|entry| !entry.claimed && entry.text.as_deref() == Some(text.as_str()))
        {
            Some(entry) => {
                entry.rules = rules;
                entry.claimed = true;
            }
            None => unmatched.push((text, rules)),
        }
    }
    let page = page_of(&page_url).to_owned();
    let mut files: Vec<(FileRun, String)> = Vec::new();
    let mut inline = 0;
    for Listed {
        href, text, rules, ..
    } in listed
    {
        let (url, is_inline) = match href {
            Some(href) => (href, false),
            None => {
                inline += 1;
                (format!("{page}#style-{inline}"), true)
            }
        };
        let Some(text) = text else {
            host.log("debug", &format!("coverage: no text for {url}"));
            continue;
        };
        files.push((css_file(url, is_inline, &text, rules), text));
    }
    for (index, (text, rules)) in unmatched.into_iter().enumerate() {
        let url = format!("{page}#sheet-{}", index + 1);
        files.push((css_file(url, true, &text, rules), text));
    }
    let mut kept: Vec<FileRun> = Vec::new();
    for (mut file, text) in files {
        if settings.skips(&file.url, &page_url) {
            continue;
        }
        file.source = store::write_source(host, &text, cache);
        match kept.iter_mut().find(|known| known.same(&file)) {
            Some(known) => known.absorb(&file),
            None => kept.push(file),
        }
    }
    Ok((page_url, kept))
}

fn js_file(url: String, inline: bool, script: &ScriptCoverage) -> FileRun {
    let counts: Vec<Count> = script
        .functions
        .iter()
        .flat_map(|function| function.ranges.iter())
        .map(|range| Count::new(range.start_offset, range.end_offset, range.count))
        .collect();
    let total = counts.iter().map(|range| range.end).max().unwrap_or(0);
    let segments = used_segments(&counts);
    let functions: Vec<Function> = script
        .functions
        .iter()
        .filter_map(|function| {
            let whole = function.ranges.first()?;
            let top = function.function_name.is_empty()
                && whole.start_offset == 0
                && whole.end_offset == total;
            (!top).then(|| Function {
                name: function.function_name.chars().take(200).collect(),
                start: whole.start_offset,
                end: whole.end_offset,
                executed: whole.count > 0,
            })
        })
        .take(MAX_FUNCTIONS)
        .collect();
    FileRun {
        url,
        kind: Kind::Js,
        inline,
        total: u64::from(total),
        used: length(&segments),
        segments,
        rules: Vec::new(),
        functions,
        source: None,
    }
}

/// The scripts of `view`'s page at `page_url`: only `http(s)` ones (no eval, no isolated
/// world), none of `skip` (an earlier document's), inline ones as `#script-<n>` of the page.
#[must_use]
pub fn js(
    scripts: &[ScriptCoverage],
    page_url: &str,
    skip: &BTreeSet<String>,
    settings: &Settings,
) -> Vec<FileRun> {
    let page = page_of(page_url);
    let mut ordered: Vec<&ScriptCoverage> = scripts
        .iter()
        .filter(|script| origin(&script.url).is_some() && !skip.contains(&script.script_id))
        .collect();
    ordered.sort_by_key(|script| {
        (
            script.script_id.parse::<u64>().unwrap_or(u64::MAX),
            script.script_id.clone(),
        )
    });
    let mut files: Vec<FileRun> = Vec::new();
    let mut inline = 0;
    for script in ordered {
        let file = if page_of(&script.url) == page {
            inline += 1;
            js_file(format!("{page}#script-{inline}"), true, script)
        } else {
            js_file(script.url.clone(), false, script)
        };
        if settings.skips(&file.url, page_url) {
            continue;
        }
        match files.iter_mut().find(|known| known.same(&file)) {
            Some(known) => known.absorb(&file),
            None => files.push(file),
        }
    }
    files
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;
