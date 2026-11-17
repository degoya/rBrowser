//! The component: `src/lib.rs` translated into the vocabulary of `wit/rbrowser-plugin.wit`. Built
//! only for `wasm32`; it holds no logic of its own, so it needs no test of its own.
#![allow(
    unsafe_code,
    reason = "the canonical-ABI glue wit-bindgen generates is the only unsafe code here"
)]

wit_bindgen::generate!({
    path: "wit",
    world: "check-plugin",
});

use exports::rbrowser::plugin::check::{CheckContext, Guest, RuleInfo};
use rbrowser::plugin::types::{Failure, Finding, Severity};
use rbrowser::plugin::{evaluate, host, views};

struct Component;

fn severity(level: crate::Level) -> Severity {
    match level {
        crate::Level::Info => Severity::Info,
        crate::Level::Warning => Severity::Warning,
        crate::Level::Error => Severity::Error,
    }
}

impl Guest for Component {
    fn describe() -> Vec<RuleInfo> {
        crate::RULES
            .iter()
            .map(|rule| RuleInfo {
                id: rule.id.to_owned(),
                title: rule.title.to_owned(),
                severity: severity(rule.severity),
                shared_id: rule.shared_id.map(str::to_owned),
                help_url: rule.help_url.map(str::to_owned),
            })
            .collect()
    }

    fn run(ctx: CheckContext) -> Result<Vec<Finding>, Failure> {
        let targets: Vec<u32> = match ctx.view {
            Some(view) => vec![view],
            None => views::list().into_iter().map(|info| info.id).collect(),
        };
        let total = u32::try_from(targets.len()).unwrap_or(u32::MAX);
        let mut findings = Vec::new();
        for (done, target) in (0..).zip(targets) {
            host::report_progress(done, total);
            let title = evaluate::evaluate(target, crate::TITLE_SCRIPT)?;
            let lang = evaluate::evaluate(target, crate::LANG_SCRIPT)?;
            for found in crate::page_findings(&ctx.rules, &title, &lang) {
                findings.push(Finding {
                    rule: found.rule.to_owned(),
                    severity: severity(found.severity),
                    message: found.message.to_owned(),
                    params: Vec::new(),
                    view: Some(target),
                    url: ctx.url.clone(),
                    selector: Some(found.selector.to_owned()),
                    path: Some(found.path.to_owned()),
                    data: None,
                });
            }
        }
        host::report_progress(total, total);
        Ok(findings)
    }
}

export!(Component);
