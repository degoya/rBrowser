//! Reading Lighthouse's JSON result (LHR): the category scores and the failed audits, and the
//! findings they become.
//!
//! An audit has failed when it is scored (`binary`, `numeric` or `metricSavings`) and its score
//! is below 0.9, Lighthouse's own pass mark. Its severity follows score and weight: an audit that
//! counts towards a category's score (weight above 0) is an error below 0.5 and a warning above;
//! one that does not is a warning below 0.5 and information above. An audit that names elements
//! becomes one finding per element (at most [`MAX_ELEMENTS`]), each with Lighthouse's selector
//! and node path, so the same element merges across the breakpoints; any other one finding for
//! the page.

use std::collections::BTreeMap;

use rb_plugin_api::{Finding, Severity, ViewId};
use serde_json::{Value, json};

use crate::key;

/// Lighthouse's pass mark.
pub const PASS: f64 = 0.9;
/// The most elements of one audit that become findings; the rest are counted in the data.
pub const MAX_ELEMENTS: usize = 20;
/// The longest snippet, explanation or label a finding carries, in characters.
const MAX_TEXT: usize = 500;

/// One element a failed audit names.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Element {
    pub selector: Option<String>,
    /// Lighthouse's node path (`1,HTML,1,BODY,3,P`).
    pub path: Option<String>,
    pub label: Option<String>,
    pub snippet: Option<String>,
    pub explanation: Option<String>,
}

/// One failed audit.
#[derive(Clone, Debug, PartialEq)]
pub struct FailedAudit {
    pub id: String,
    pub title: String,
    pub score: f64,
    pub display_value: Option<String>,
    /// The highest weight of the audit in the run's categories.
    pub weight: f64,
    pub categories: Vec<String>,
    pub help_url: Option<String>,
    pub elements: Vec<Element>,
    /// Elements past [`MAX_ELEMENTS`].
    pub more_elements: usize,
}

/// What a run measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub lighthouse_version: String,
    pub requested_url: String,
    pub final_url: String,
    /// Lighthouse's code when the page could not be measured (`ERRORED_DOCUMENT_REQUEST`).
    pub runtime_error: Option<String>,
    pub form_factor: Option<String>,
    pub width: Option<u64>,
    /// Category id -> score 0 to 100, `None` when Lighthouse gave none.
    pub scores: BTreeMap<String, Option<u8>>,
    /// Lighthouse's metrics (`crate::history::METRICS`) by audit id, their `numericValue`
    /// (RB-091-02: the history compares them).
    pub metrics: BTreeMap<String, Value>,
    pub failed: Vec<FailedAudit>,
}

fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

fn cut(value: &Value) -> Option<String> {
    text(value).map(|text| text.chars().take(MAX_TEXT).collect())
}

/// The link of "[Learn more ...](https://...)" in an audit's description.
fn help_url(description: &str) -> Option<String> {
    let start = description.rfind("](http")? + 2;
    let end = start + description[start..].find(')')?;
    Some(description[start..end].to_owned())
}

/// The first node of an item of a details table.
fn node_of(item: &Value) -> Option<Element> {
    let node = item
        .as_object()?
        .values()
        .find(|value| value["type"] == "node")?;
    Some(Element {
        selector: cut(&node["selector"]),
        path: cut(&node["path"]),
        label: cut(&node["nodeLabel"]),
        snippet: cut(&node["snippet"]),
        explanation: cut(&node["explanation"]),
    })
}

fn elements(details: &Value) -> Vec<Element> {
    let items = details["items"].as_array().map_or(&[][..], Vec::as_slice);
    items
        .iter()
        .flat_map(|item| {
            // A `list` holds tables of its own.
            if item["type"] == "table" {
                item["items"].as_array().cloned().unwrap_or_default()
            } else {
                vec![item.clone()]
            }
        })
        .filter_map(|item| node_of(&item))
        .collect()
}

fn scored(mode: &str) -> bool {
    matches!(mode, "binary" | "numeric" | "metricSavings")
}

/// Reads an LHR.
///
/// # Errors
///
/// The text is no JSON object with `categories` and `audits` (the reason).
pub fn parse(text_of_lhr: &str) -> Result<Report, String> {
    let lhr: Value = serde_json::from_str(text_of_lhr).map_err(|error| error.to_string())?;
    let (Some(categories), Some(audits)) =
        (lhr["categories"].as_object(), lhr["audits"].as_object())
    else {
        return Err("no categories or audits".to_owned());
    };
    let mut scores = BTreeMap::new();
    let mut weights: BTreeMap<String, (f64, Vec<String>)> = BTreeMap::new();
    for (id, category) in categories {
        let score = category["score"]
            .as_f64()
            .map(|score| (score.clamp(0.0, 1.0) * 100.0).round() as u8);
        scores.insert(id.clone(), score);
        for reference in category["auditRefs"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
        {
            let Some(audit) = reference["id"].as_str() else {
                continue;
            };
            let entry = weights.entry(audit.to_owned()).or_default();
            entry.0 = entry.0.max(reference["weight"].as_f64().unwrap_or(0.0));
            entry.1.push(id.clone());
        }
    }
    let mut failed = Vec::new();
    for (id, (weight, in_categories)) in weights {
        let audit = &audits[&id];
        let (Some(score), Some(mode)) =
            (audit["score"].as_f64(), audit["scoreDisplayMode"].as_str())
        else {
            continue;
        };
        if !scored(mode) || score >= PASS {
            continue;
        }
        let mut found = elements(&audit["details"]);
        let more_elements = found.len().saturating_sub(MAX_ELEMENTS);
        found.truncate(MAX_ELEMENTS);
        failed.push(FailedAudit {
            title: text(&audit["title"]).unwrap_or_else(|| id.clone()),
            id,
            score,
            display_value: text(&audit["displayValue"]),
            weight,
            categories: in_categories,
            help_url: audit["description"].as_str().and_then(help_url),
            elements: found,
            more_elements,
        });
    }
    let settings = &lhr["configSettings"];
    Ok(Report {
        lighthouse_version: text(&lhr["lighthouseVersion"]).unwrap_or_default(),
        requested_url: text(&lhr["requestedUrl"]).unwrap_or_default(),
        final_url: text(&lhr["finalDisplayedUrl"]).unwrap_or_default(),
        runtime_error: text(&lhr["runtimeError"]["code"]),
        form_factor: text(&settings["formFactor"]),
        width: settings["screenEmulation"]["width"].as_u64(),
        scores,
        metrics: crate::history::metrics_of_lhr(&lhr),
        failed,
    })
}

/// The severity of a failed audit.
#[must_use]
pub fn severity(score: f64, weight: f64) -> Severity {
    match (weight > 0.0, score < 0.5) {
        (true, true) => Severity::Error,
        (true, false) | (false, true) => Severity::Warning,
        (false, false) => Severity::Info,
    }
}

fn data(audit: &FailedAudit, element: Option<&Element>) -> String {
    let mut data = json!({
        "score": audit.score,
        "weight": audit.weight,
        "categories": audit.categories,
    });
    if let Some(value) = &audit.display_value {
        data["displayValue"] = json!(value);
    }
    if let Some(url) = &audit.help_url {
        data["helpUrl"] = json!(url);
    }
    if audit.more_elements > 0 {
        data["moreElements"] = json!(audit.more_elements);
    }
    if let Some(element) = element {
        if let Some(snippet) = &element.snippet {
            data["snippet"] = json!(snippet);
        }
        if let Some(explanation) = &element.explanation {
            data["explanation"] = json!(explanation);
        }
    }
    data.to_string()
}

/// The findings of `report`'s failed audits, measured in `view` at `url`, of the rules
/// `enabled` lets through.
#[must_use]
pub fn findings(
    report: &Report,
    view: ViewId,
    url: &str,
    enabled: &dyn Fn(&str) -> bool,
) -> Vec<Finding> {
    let mut out = Vec::new();
    for audit in report.failed.iter().filter(|audit| enabled(&audit.id)) {
        let base = Finding {
            rule: audit.id.clone(),
            severity: severity(audit.score, audit.weight),
            message: String::new(),
            params: vec![("title".to_owned(), audit.title.clone())],
            view: Some(view),
            url: Some(url.to_owned()),
            selector: None,
            path: None,
            data: None,
        };
        if audit.elements.is_empty() {
            let mut finding = base;
            match &audit.display_value {
                Some(value) => {
                    finding.message = key("findings", "audit_value");
                    finding.params.push(("value".to_owned(), value.clone()));
                }
                None => finding.message = key("findings", "audit"),
            }
            finding.data = Some(data(audit, None));
            out.push(finding);
            continue;
        }
        for element in &audit.elements {
            let mut finding = base.clone();
            finding.message = key("findings", "element");
            let name = element
                .label
                .clone()
                .or_else(|| element.selector.clone())
                .unwrap_or_default();
            finding.params.push(("element".to_owned(), name));
            finding.selector.clone_from(&element.selector);
            finding.path.clone_from(&element.path);
            finding.data = Some(data(audit, Some(element)));
            out.push(finding);
        }
    }
    out
}

#[cfg(test)]
#[path = "lhr_tests.rs"]
mod tests;
