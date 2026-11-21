//! A check run as JUnit XML: one `testsuite` per breakpoint (per checked view without one, in the
//! order of the run's views), in it one `testcase` per rule with findings there - one `failure`
//! naming them all, of the type of the highest severity - an `error` per plugin that failed in
//! one of its views, a `skipped` per plugin that could not check one, and a passing case per
//! plugin that found nothing there. What concerns no view is the suite "every view".

use std::collections::BTreeMap;
use std::fmt::Write as _;

use rb_plugin_types::{FindingMessage, FindingsFile, MergedFinding, html::escape};

use super::{ExportContext, sarif};

/// How a test case ended.
enum Outcome {
    Passed,
    Failure {
        message: String,
        kind: &'static str,
        body: String,
    },
    Error(String),
    Skipped(String),
}

struct Case {
    classname: String,
    name: String,
    outcome: Outcome,
}

struct Suite {
    name: String,
    /// The views of the suite (empty: every view).
    views: Vec<u32>,
    cases: Vec<Case>,
}

impl Suite {
    fn count(&self, kind: fn(&Outcome) -> bool) -> usize {
        self.cases.iter().filter(|case| kind(&case.outcome)).count()
    }
}

/// `text` safe in XML 1.0 text and quoted attributes: the characters XML forbids dropped, the
/// markup characters escaped.
pub fn xml(text: &str) -> String {
    let allowed: String = text
        .chars()
        .filter(|c| {
            !(c.is_control() && !matches!(c, '\t' | '\n' | '\r'))
                && !matches!(c, '\u{fffe}' | '\u{ffff}')
        })
        .collect();
    escape(&allowed)
}

/// The suites of `file`: the checked views' breakpoints first, then "every view" if needed.
fn suites_of(file: &FindingsFile, context: &ExportContext<'_>) -> Vec<Suite> {
    let mut suites: Vec<Suite> = Vec::new();
    for checked in &file.views {
        let name = context.view_name(file, checked.id);
        match suites.iter().position(|suite| suite.name == name) {
            Some(at) => {
                if let Some(suite) = suites.get_mut(at) {
                    suite.views.push(checked.id);
                }
            }
            None => suites.push(Suite {
                name,
                views: vec![checked.id],
                cases: Vec::new(),
            }),
        }
    }
    suites
}

/// The index of the suite of `view` (`None`: every view), made when missing.
fn suite_for(
    suites: &mut Vec<Suite>,
    view: Option<u32>,
    file: &FindingsFile,
    context: &ExportContext<'_>,
) -> usize {
    let found = match view {
        Some(view) => suites.iter().position(|suite| suite.views.contains(&view)),
        None => suites.iter().position(|suite| suite.views.is_empty()),
    };
    found.unwrap_or_else(|| {
        let name = view.map_or_else(
            || (context.text)("all_views", &[]),
            |view| context.view_name(file, view),
        );
        suites.push(Suite {
            name,
            views: view.into_iter().collect(),
            cases: Vec::new(),
        });
        suites.len() - 1
    })
}

fn failure_of(findings: &[&MergedFinding], context: &ExportContext<'_>) -> Outcome {
    let worst = findings
        .iter()
        .map(|finding| finding.severity)
        .max()
        .unwrap_or_default();
    let message = match findings {
        [one] => context.message(&one.message),
        many => (context.text)("count", &[("count", &many.len().to_string())]),
    };
    let mut body = String::new();
    for finding in findings {
        let _ = write!(
            body,
            "{}: {}",
            finding.severity.as_str(),
            context.message(&finding.message)
        );
        for detail in [
            finding.url.as_deref(),
            finding
                .element
                .as_ref()
                .and_then(|element| element.selector.as_deref()),
        ]
        .into_iter()
        .flatten()
        {
            let _ = write!(body, "\n  {detail}");
        }
        body.push('\n');
    }
    Outcome::Failure {
        message,
        kind: worst.as_str(),
        body,
    }
}

fn add_findings(suites: &mut Vec<Suite>, file: &FindingsFile, context: &ExportContext<'_>) {
    // Per suite, the rules in the order they first show, with their findings.
    let mut by_suite: BTreeMap<usize, Vec<(&str, Vec<&MergedFinding>)>> = BTreeMap::new();
    for finding in &file.findings {
        let mut targets: Vec<usize> = Vec::new();
        if finding.views.is_empty() {
            targets.push(suite_for(suites, None, file, context));
        }
        for view in &finding.views {
            let index = suite_for(suites, Some(*view), file, context);
            if !targets.contains(&index) {
                targets.push(index);
            }
        }
        for index in targets {
            let rules = by_suite.entry(index).or_default();
            match rules.iter().position(|(rule, _)| *rule == finding.rule_id) {
                Some(at) => {
                    if let Some((_, found)) = rules.get_mut(at) {
                        found.push(finding);
                    }
                }
                None => rules.push((finding.rule_id.as_str(), vec![finding])),
            }
        }
    }
    for (index, rules) in by_suite {
        for (rule, findings) in rules {
            let classname = findings
                .first()
                .and_then(|finding| finding.plugins.first())
                .cloned()
                .unwrap_or_default();
            let outcome = failure_of(&findings, context);
            if let Some(suite) = suites.get_mut(index) {
                suite.cases.push(Case {
                    classname,
                    name: rule.to_owned(),
                    outcome,
                });
            }
        }
    }
}

fn add_failures_and_skips(
    suites: &mut Vec<Suite>,
    file: &FindingsFile,
    context: &ExportContext<'_>,
) {
    for failure in &file.failures {
        let index = suite_for(suites, failure.view, file, context);
        let outcome = Outcome::Error(sarif::failure_text(failure, context));
        if let Some(suite) = suites.get_mut(index) {
            suite.cases.push(Case {
                classname: failure.plugin.clone(),
                name: failure.plugin.clone(),
                outcome,
            });
        }
    }
    for skip in &file.skipped {
        let index = suite_for(suites, skip.view, file, context);
        let reason = context.message(&FindingMessage {
            key: format!("checks.skip.{}", skip.reason),
            params: skip
                .engine
                .iter()
                .map(|engine| ("engine".to_owned(), engine.clone()))
                .collect(),
        });
        let message = (context.text)("skipped", &[("plugin", &skip.plugin), ("reason", &reason)]);
        if let Some(suite) = suites.get_mut(index) {
            suite.cases.push(Case {
                classname: skip.plugin.clone(),
                name: skip.plugin.clone(),
                outcome: Outcome::Skipped(message),
            });
        }
    }
}

/// A passing case per plugin with nothing in a view's suite.
fn add_passes(suites: &mut [Suite], file: &FindingsFile) {
    for suite in suites.iter_mut().filter(|suite| !suite.views.is_empty()) {
        for plugin in &file.plugins {
            if !suite.cases.iter().any(|case| &case.classname == plugin) {
                suite.cases.push(Case {
                    classname: plugin.clone(),
                    name: plugin.clone(),
                    outcome: Outcome::Passed,
                });
            }
        }
    }
}

fn write_case(out: &mut String, case: &Case) {
    let _ = write!(
        out,
        "<testcase classname=\"{}\" name=\"{}\"",
        xml(&case.classname),
        xml(&case.name)
    );
    let _ = match &case.outcome {
        Outcome::Passed => write!(out, "/>"),
        Outcome::Failure {
            message,
            kind,
            body,
        } => write!(
            out,
            "><failure message=\"{}\" type=\"{kind}\">{}</failure></testcase>",
            xml(message),
            xml(body)
        ),
        Outcome::Error(message) => write!(
            out,
            "><error message=\"{}\" type=\"plugin\"/></testcase>",
            xml(message)
        ),
        Outcome::Skipped(message) => {
            write!(out, "><skipped message=\"{}\"/></testcase>", xml(message))
        }
    };
    out.push('\n');
}

fn is_failure(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Failure { .. })
}

fn is_error(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Error(_))
}

fn is_skipped(outcome: &Outcome) -> bool {
    matches!(outcome, Outcome::Skipped(_))
}

/// The JUnit XML of `file`.
pub fn render(file: &FindingsFile, context: &ExportContext<'_>) -> String {
    let mut suites = suites_of(file, context);
    add_findings(&mut suites, file, context);
    add_failures_and_skips(&mut suites, file, context);
    add_passes(&mut suites, file);
    let total =
        |kind: fn(&Outcome) -> bool| suites.iter().map(|suite| suite.count(kind)).sum::<usize>();
    let tests: usize = suites.iter().map(|suite| suite.cases.len()).sum();
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        out,
        "<testsuites name=\"{} {}\" tests=\"{tests}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\">",
        xml(&(context.text)("title", &[])),
        xml(&file.run_id),
        total(is_failure),
        total(is_error),
        total(is_skipped)
    );
    for suite in &suites {
        let _ = writeln!(
            out,
            "<testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\">",
            xml(&suite.name),
            suite.cases.len(),
            suite.count(is_failure),
            suite.count(is_error),
            suite.count(is_skipped)
        );
        let mut engines: Vec<&str> = suite
            .views
            .iter()
            .filter_map(|view| context.engines.get(view).map(String::as_str))
            .collect();
        engines.dedup();
        let _ = writeln!(
            out,
            "<properties><property name=\"runId\" value=\"{}\"/><property name=\"engines\" value=\"{}\"/></properties>",
            xml(&file.run_id),
            xml(&engines.join(","))
        );
        for case in &suite.cases {
            write_case(&mut out, case);
        }
        out.push_str("</testsuite>\n");
    }
    out.push_str("</testsuites>\n");
    out
}

#[cfg(test)]
#[path = "junit_tests.rs"]
mod tests;
