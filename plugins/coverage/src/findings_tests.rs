use std::collections::BTreeMap;

use super::{FUNCTION_UNUSED, RULE_PARTIAL, UNUSED_FILE, describe, findings};
use crate::model::{FileRun, Function, Kind, Mode, Run, Status, ViewRun};
use crate::settings::Settings;

const TEXT: &str = "body { margin: 0 }\n@media (min-width: 1024px) { .wide { display: grid } }\n";

fn site(wide: bool) -> FileRun {
    let rules = if wide {
        vec![(0, 18), (48, 71)]
    } else {
        vec![(0, 18)]
    };
    FileRun {
        url: "http://localhost:8080/coverage/site.css".to_owned(),
        kind: Kind::Css,
        total: 74,
        used: if wide { 41 } else { 18 },
        segments: rules.clone(),
        rules,
        source: Some("k".to_owned()),
        ..FileRun::default()
    }
}

fn unused() -> FileRun {
    FileRun {
        url: "http://localhost:8080/coverage/unused.css".to_owned(),
        kind: Kind::Css,
        total: 20_480,
        used: 0,
        ..FileRun::default()
    }
}

fn view(id: u32, breakpoint: &str, files: Vec<FileRun>) -> ViewRun {
    ViewRun {
        view: id,
        label: format!("View {id}"),
        breakpoint: breakpoint.to_owned(),
        width: 0,
        engine: "chromium".to_owned(),
        url: "http://localhost:8080/coverage/index.html#top".to_owned(),
        status: Status::Measured,
        error: None,
        files,
    }
}

fn run(mode: Mode) -> Run {
    Run {
        mode,
        views: vec![
            view(1, "desktop", vec![site(true), unused()]),
            view(2, "laptop", vec![site(true), unused()]),
            view(3, "mobile", vec![site(false), unused()]),
            ViewRun {
                status: Status::Unavailable,
                ..view(4, "firefox", Vec::new())
            },
        ],
        ..Run::default()
    }
}

fn texts() -> BTreeMap<String, String> {
    [("k".to_owned(), TEXT.to_owned())].into()
}

#[test]
fn a_rule_used_only_on_wide_breakpoints_names_exactly_those() {
    let found = findings(&run(Mode::Load), &Settings::default(), &[], None, &texts());
    let partial: Vec<_> = found.iter().filter(|f| f.rule == RULE_PARTIAL).collect();
    assert_eq!(partial.len(), 2, "one per view that uses it");
    let views: Vec<_> = partial.iter().filter_map(|f| f.view).collect();
    assert_eq!(views, [1, 2]);
    let param = |name: &str| {
        partial[0]
            .params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    assert_eq!(param("breakpoints").as_deref(), Some("desktop, laptop"));
    assert_eq!(param("unused").as_deref(), Some("mobile"));
    assert_eq!(param("selector").as_deref(), Some(".wide"));
    assert_eq!(
        partial[0].path.as_deref(),
        Some("http://localhost:8080/coverage/site.css#48-71")
    );
    assert_eq!(
        partial[0].url.as_deref(),
        Some("http://localhost:8080/coverage/index.html")
    );
    assert_eq!(partial[0].message, "plugin.coverage.findings.rule-partial");
}

#[test]
fn a_never_used_large_file_is_a_warning_at_100_percent() {
    let found = findings(&run(Mode::Load), &Settings::default(), &[], None, &texts());
    let unused: Vec<_> = found.iter().filter(|f| f.rule == UNUSED_FILE).collect();
    assert_eq!(unused.len(), 3, "one per measured view");
    let hundred = ("percent".to_owned(), "100".to_owned());
    assert!(unused.iter().all(|f| f.params.contains(&hundred)));
    assert_eq!(unused[0].severity, rb_plugin_api::Severity::Warning);
    // site.css is small: no warning, whatever its share.
    let site = Some("http://localhost:8080/coverage/site.css");
    assert!(
        !found
            .iter()
            .any(|f| f.rule == UNUSED_FILE && f.path.as_deref() == site)
    );
}

#[test]
fn rules_switched_off_and_other_views_give_nothing() {
    let only = [UNUSED_FILE.to_owned()];
    let found = findings(
        &run(Mode::Load),
        &Settings::default(),
        &only,
        Some(3),
        &texts(),
    );
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].view, Some(3));
    let none = findings(
        &run(Mode::Load),
        &Settings::default(),
        &[],
        Some(4),
        &texts(),
    );
    assert!(none.is_empty());
}

#[test]
fn functions_no_view_ran_are_reported_after_an_interaction_only() {
    let script = |executed| FileRun {
        url: "http://localhost:8080/coverage/app.js".to_owned(),
        kind: Kind::Js,
        total: 100,
        used: 100,
        functions: vec![
            Function {
                name: "onClick".to_owned(),
                start: 10,
                end: 40,
                executed,
            },
            Function {
                name: String::new(),
                start: 50,
                end: 60,
                executed: false,
            },
        ],
        ..FileRun::default()
    };
    let mut interaction = Run {
        mode: Mode::Interaction,
        views: vec![
            view(1, "desktop", vec![script(true)]),
            view(2, "mobile", vec![script(false)]),
        ],
        ..Run::default()
    };
    let none = BTreeMap::new();
    let found = findings(&interaction, &Settings::default(), &[], None, &none);
    let names: Vec<String> = found
        .iter()
        .filter(|f| f.rule == FUNCTION_UNUSED)
        .filter_map(|f| {
            f.params
                .iter()
                .find(|(k, _)| k == "name")
                .map(|(_, v)| v.clone())
        })
        .collect();
    assert_eq!(names, ["(anonymous)", "(anonymous)"]);
    interaction.mode = Mode::Load;
    assert!(findings(&interaction, &Settings::default(), &[], None, &none).is_empty());
}

#[test]
fn every_rule_is_described_with_its_title_key() {
    let rules = describe();
    let ids: Vec<&str> = rules.iter().map(|rule| rule.id.as_str()).collect();
    assert_eq!(ids, [UNUSED_FILE, RULE_PARTIAL, FUNCTION_UNUSED]);
    assert_eq!(rules[0].title, "plugin.coverage.rules.unused-file");
}
