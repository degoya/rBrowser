//! The check (`check-plugin`, rule group `errors`): one rule per category, a finding per entry and
//! view. All findings of one entry share the `path` `error-report/<entry id>`, so the host's check
//! runner merges them into one finding with every view that reported it; an entry not every
//! capturing view reported says so in its message ("only in views: mobile").

use rb_plugin_api::{Finding, RuleInfo, ViewId};

use crate::classify::Category;
use crate::key;
use crate::normalize::cut;
use crate::report::{Report, entry_json};

/// The longest text a finding quotes.
pub const MAX_QUOTE_CHARS: usize = 300;
/// The longest `data` of a finding (the host's limit is 64 KiB).
pub const MAX_DATA_CHARS: usize = 60_000;

/// The rules `describe` answers, one per category.
#[must_use]
pub fn rules() -> Vec<RuleInfo> {
    Category::ALL
        .into_iter()
        .map(|category| RuleInfo {
            id: category.rule().to_owned(),
            title: key(&format!("rules.{}", category.key_name())),
            severity: category.severity(),
            shared_id: None,
            help_url: None,
        })
        .collect()
}

/// The findings of `report` among the rules `wanted` (every rule when empty), for `view` only
/// when one is named.
#[must_use]
pub fn findings(report: &Report, view: Option<ViewId>, wanted: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for entry in &report.entries {
        let rule = entry.category.rule();
        if !wanted.is_empty() && !wanted.iter().any(|id| id == rule) {
            continue;
        }
        let name = entry.category.key_name();
        let message = match entry.only_in {
            Some(_) => key(&format!("findings.{name}_only")),
            None => key(&format!("findings.{name}")),
        };
        let source = entry
            .url
            .clone()
            .or_else(|| entry.source.as_ref().map(|s| s.url.clone()))
            .unwrap_or_else(|| "?".to_owned());
        let mut params = vec![
            ("text".to_owned(), cut(&entry.text, MAX_QUOTE_CHARS)),
            ("source".to_owned(), cut(&source, MAX_QUOTE_CHARS)),
        ];
        if let Some(names) = &entry.only_in {
            params.push(("views".to_owned(), cut(&names.join(", "), MAX_QUOTE_CHARS)));
        }
        let data = entry_json(entry).to_string();
        let data = (data.chars().count() <= MAX_DATA_CHARS).then_some(data);
        for &reporting in entry.per_view.keys() {
            if view.is_some_and(|view| view != reporting) {
                continue;
            }
            let page = report
                .views
                .iter()
                .find(|v| v.id == reporting)
                .map(|v| v.url.clone())
                .filter(|url| !url.is_empty());
            out.push(Finding {
                rule: rule.to_owned(),
                severity: entry.severity,
                message: message.clone(),
                params: params.clone(),
                view: Some(reporting),
                url: page,
                selector: None,
                path: Some(format!("error-report/{}", entry.id)),
                data: data.clone(),
            });
        }
    }
    out
}

#[cfg(test)]
#[path = "findings_tests.rs"]
mod tests;
