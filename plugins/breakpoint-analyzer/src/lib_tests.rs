//! The plugin through its native traits, against `MemoryHost` (no capability granted), and the
//! K5 snapshots: the built CSS of `tests/fixtures/analyzer/` in every mode, and with the
//! fixtures' project sources (RB-030-07) the static source beside it.

use rb_plugin_api::analyzer::Analyzer;
use rb_plugin_api::feature::Feature;
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{FailureKind, codes};
use serde_json::{Value, json};

use super::{BreakpointAnalyzer, DetectArgs, Sheet, detect};
use crate::modes::Mode;
use crate::static_source::ProjectFile;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/fixtures/analyzer");

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{FIXTURES}/{path}"))
        .unwrap_or_else(|error| panic!("fixture {path}: {error}"))
}

/// The stylesheets of a fixture as the host would hand them over.
fn sheets(fixture: &str) -> Vec<Sheet> {
    let files: &[&str] = match fixture {
        "tailwind3" => &["tailwind3/tailwind.css"],
        "tailwind4" => &["tailwind4/app.css"],
        "bootstrap" => &["bootstrap/css/bootstrap.min.css"],
        _ => &[
            "cross-origin/remote/remote.css",
            "cross-origin/remote/wide.css",
        ],
    };
    let mut sheets: Vec<Sheet> = files
        .iter()
        .map(|file| Sheet {
            url: format!("http://fixture/{file}"),
            text: read(file),
            media: None,
        })
        .collect();
    if fixture == "cross-origin" {
        sheets.push(Sheet {
            url: "http://fixture/cross-origin/index.html#inline-1".to_owned(),
            text: "@media (min-width: 600px) { main { padding: 2rem; } }".to_owned(),
            media: None,
        });
    }
    sheets
}

fn summary(fixture: &str, mode: Mode) -> Value {
    let args = DetectArgs {
        sheets: sheets(fixture),
        mode,
        ..DetectArgs::default()
    };
    let (detection, _) = detect(&args);
    let views: Vec<Value> = detection
        .breakpoints
        .iter()
        .map(|view| json!([view.id, view.width, view.height]))
        .collect();
    Value::Array(views)
}

#[test]
fn k5_every_fixture_in_every_mode_matches_its_snapshot() {
    for fixture in ["tailwind3", "tailwind4", "bootstrap", "cross-origin"] {
        let expected: Value =
            serde_json::from_str(&read(&format!("{fixture}/expected.json"))).expect("json");
        for (mode, name) in [
            (Mode::All, "all"),
            (Mode::Simplified, "simplified"),
            (Mode::Edges, "edges"),
        ] {
            assert_eq!(
                summary(fixture, mode),
                expected[name],
                "{fixture} in mode {name}"
            );
        }
        let (detection, _) = detect(&DetectArgs {
            sheets: sheets(fixture),
            ..DetectArgs::default()
        });
        let detected: Vec<Value> = detection
            .detected
            .iter()
            .map(|found| json!([found.width, found.kind, found.label]))
            .collect();
        assert_eq!(Value::Array(detected), expected["detected"], "{fixture}");
        assert_eq!(
            json!(detection.framework),
            expected["framework"],
            "{fixture}"
        );
    }
}

#[test]
fn the_tolerance_merges_767_768_and_770() {
    let css =
        "@media (min-width: 767px) {} @media (min-width: 768px) {} @media (min-width: 770px) {}";
    let args = |tolerance| DetectArgs {
        sheets: vec![Sheet {
            text: css.to_owned(),
            ..Sheet::default()
        }],
        merge_tolerance_px: Some(tolerance),
        ..DetectArgs::default()
    };
    assert_eq!(detect(&args(8)).0.detected.len(), 1);
    assert_eq!(detect(&args(8)).0.detected[0].count, 3);
    assert_eq!(detect(&args(0)).0.detected.len(), 3);
}

#[test]
fn sources_count_their_queries_and_container_queries_are_a_hint() {
    let (detection, _) = detect(&DetectArgs {
        sheets: sheets("tailwind4"),
        ..DetectArgs::default()
    });
    assert_eq!(detection.sources.len(), 1);
    assert_eq!(detection.sources[0].count, 8);
    assert_eq!(detection.container_queries, 1);
}

#[test]
fn known_widths_are_not_new() {
    let (detection, _) = detect(&DetectArgs {
        sheets: sheets("tailwind3"),
        existing: vec![super::Existing {
            id: "tablet".to_owned(),
            width: 768,
        }],
        ..DetectArgs::default()
    });
    let new: Vec<&str> = detection
        .breakpoints
        .iter()
        .filter(|view| view.is_new)
        .map(|view| view.id.as_str())
        .collect();
    assert_eq!(new, vec!["mobile", "sm", "lg", "xl", "2xl", "desktop"]);
}

#[test]
fn analyze_proposes_each_new_view_without_any_capability() {
    let mut host = MemoryHost::new();
    let args = json!({
        "sheets": [{ "url": "u", "text": read("bootstrap/css/bootstrap.min.css") }],
        "existing": [{ "id": "desktop", "width": 1440 }]
    });
    let proposals = BreakpointAnalyzer
        .analyze(&mut host, &args.to_string())
        .expect("analysis");
    assert_eq!(proposals.len(), 6);
    assert!(
        proposals
            .iter()
            .all(|proposal| proposal.pointer == "/breakpoints/-")
    );
    assert_eq!(
        proposals[0].reason,
        "plugin.breakpoint-analyzer.reasons.mobile"
    );
    assert_eq!(
        proposals[1].reason,
        "plugin.breakpoint-analyzer.reasons.boundary"
    );
    assert_eq!(
        proposals[1].params,
        vec![
            ("width".to_owned(), "576".to_owned()),
            ("count".to_owned(), "3".to_owned())
        ]
    );
    let value: Value = serde_json::from_str(&proposals[1].value).expect("json");
    assert_eq!(
        value,
        json!({ "id": "sm", "label": "sm", "width": 576, "height": 1024 })
    );
    // Only the log; nothing the plugin was not granted.
    assert!(host.cdp_sent.is_empty() && host.fetched.is_empty() && host.out.is_empty());
}

#[test]
fn the_tools_answer_json() {
    let mut host = MemoryHost::new();
    let detected = BreakpointAnalyzer
        .call_tool(
            &mut host,
            "detect",
            &json!({ "sheets": [{ "text": "@media (min-width: 700px) {}" }], "mode": "edges" })
                .to_string(),
        )
        .expect("detect");
    let detected: Value = serde_json::from_str(&detected).expect("json");
    assert_eq!(
        detected["detected"][0],
        json!({ "width": 700, "kind": "min", "count": 1 })
    );
    assert_eq!(detected["breakpoints"][1]["isNew"], true);
    assert_eq!(detected["containerQueries"], 0);

    let applied = BreakpointAnalyzer
        .call_tool(
            &mut host,
            "apply",
            &json!({ "proposed": detected["breakpoints"], "replace": true }).to_string(),
        )
        .expect("apply");
    let applied: Value = serde_json::from_str(&applied).expect("json");
    assert_eq!(applied["added"], json!(["w700-1", "w700"]));
    assert_eq!(applied["layout"]["columns"], 2);
}

#[test]
fn unknown_tools_commands_and_bad_arguments_fail_with_a_code() {
    let mut host = MemoryHost::new();
    let unknown = BreakpointAnalyzer
        .call_tool(&mut host, "nope", "{}")
        .expect_err("unknown");
    assert_eq!(unknown.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let command = BreakpointAnalyzer
        .run_command(&mut host, "detect", "{}")
        .expect_err("no commands");
    assert_eq!(command.code(), Some(codes::UNKNOWN_CONTRIBUTION));
    let bad = BreakpointAnalyzer
        .analyze(&mut host, "{\"mode\": \"sideways\"}")
        .expect_err("bad mode");
    assert_eq!(bad.kind, FailureKind::BadData);
    assert_eq!(
        bad.code(),
        Some("plugin.breakpoint-analyzer.errors.bad_args")
    );
    assert!(BreakpointAnalyzer.activate(&mut host).is_ok());
}

/// The project files of a fixture as the host hands them over: Tailwind configs, CSS, SCSS and
/// Less, by path relative to the fixture.
fn project_files(fixture: &str) -> Vec<ProjectFile> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, found: &mut Vec<ProjectFile>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .expect("fixture folder")
            .flatten()
            .map(|entry| entry.path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, found);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let wanted = name.starts_with("tailwind.config.")
                || [".css", ".scss", ".less"]
                    .iter()
                    .any(|extension| name.ends_with(extension));
            if wanted {
                let relative = path.strip_prefix(root).expect("below the root");
                found.push(ProjectFile {
                    path: relative.to_string_lossy().replace('\\', "/"),
                    text: std::fs::read_to_string(&path).expect("text"),
                });
            }
        }
    }
    let root = std::path::PathBuf::from(FIXTURES).join(fixture);
    let mut found = Vec::new();
    walk(&root, &root, &mut found);
    found
}

#[test]
fn static_k5_fixture_sources_beside_the_stylesheets_match_their_snapshot() {
    for fixture in ["tailwind3", "tailwind4", "bootstrap"] {
        let expected: Value =
            serde_json::from_str(&read(&format!("{fixture}/expected-static.json"))).expect("json");
        let (detection, _) = detect(&DetectArgs {
            sheets: sheets(fixture),
            files: project_files(fixture),
            ..DetectArgs::default()
        });
        let views: Vec<Value> = detection
            .breakpoints
            .iter()
            .map(|view| json!([view.id, view.width, view.height]))
            .collect();
        assert_eq!(Value::Array(views), expected["all"], "{fixture}");
        let detected: Vec<Value> = detection
            .detected
            .iter()
            .map(|found| json!([found.width, found.kind, found.label, found.source]))
            .collect();
        assert_eq!(Value::Array(detected), expected["detected"], "{fixture}");
        let files: Vec<Value> = detection
            .sources
            .iter()
            .filter(|source| !source.url.starts_with("http://"))
            .map(|source| json!([source.url, source.count]))
            .collect();
        assert_eq!(Value::Array(files), expected["sources"], "{fixture}");
        assert_eq!(
            json!(detection.framework),
            expected["framework"],
            "{fixture}"
        );
        assert!(
            detection.hints.is_empty(),
            "{fixture}: {:?}",
            detection.hints
        );
    }
}

#[test]
fn static_off_means_no_files_and_the_run_time_snapshot() {
    // `analyzer.static: false`: the host hands over no files; the proposal is the run-time one.
    let without = summary("tailwind3", Mode::All);
    let expected: Value = serde_json::from_str(&read("tailwind3/expected.json")).expect("json");
    assert_eq!(without, expected["all"]);
    let (with, _) = detect(&DetectArgs {
        sheets: sheets("tailwind3"),
        files: project_files("tailwind3"),
        ..DetectArgs::default()
    });
    assert_eq!(with.breakpoints.len(), 8, "3xl and a wider desktop view");
    assert!(without.as_array().is_some_and(|views| views.len() == 7));
}

#[test]
fn static_only_declared_widths_are_proposed_with_their_reason_and_hints_are_answered() {
    let mut host = MemoryHost::new();
    let args = json!({
        "files": [
            { "path": "tailwind.config.js",
              "text": "module.exports = { theme: { screens: { tablet: '700px', wide: wide() } } }" }
        ]
    });
    let proposals = BreakpointAnalyzer
        .analyze(&mut host, &args.to_string())
        .expect("analysis");
    let tablet = proposals
        .iter()
        .find(|proposal| proposal.value.contains("\"tablet\""))
        .expect("tablet");
    assert_eq!(tablet.reason, "plugin.breakpoint-analyzer.reasons.declared");
    assert_eq!(
        tablet.params[1],
        ("source".to_owned(), "tailwind.config.js: tablet".to_owned())
    );
    let answer = BreakpointAnalyzer
        .call_tool(&mut host, "detect", &args.to_string())
        .expect("detect");
    let answer: Value = serde_json::from_str(&answer).expect("json");
    assert_eq!(
        answer["detected"][0],
        json!({ "width": 700, "kind": "min", "label": "tablet", "count": 0,
                "source": "tailwind.config.js: tablet" })
    );
    assert_eq!(answer["framework"], "tailwind");
    assert_eq!(
        answer["hints"],
        json!([{ "path": "tailwind.config.js", "key": "plugin.breakpoint-analyzer.hints.dynamic",
                 "params": { "name": "wide", "path": "tailwind.config.js" } }])
    );
    assert_eq!(
        answer["sources"],
        json!([{ "url": "tailwind.config.js", "count": 1 }])
    );
}

#[test]
fn static_unocss_names_reach_every_boundary_of_the_page_from_2xl_on_too() {
    // RB-090-50: a UnoCSS page (`presetWind4`) with its own breakpoints. Its queries hit three
    // Tailwind widths, so without the config the boundaries are named after Tailwind - one step
    // off - and those from 1600 on not at all; the footer showed no class there.
    let text: String = [768, 1024, 1280, 1600, 1920, 2000]
        .iter()
        .map(|width| format!("@media (min-width: {width}px){{a{{b:c}}}}\n"))
        .collect();
    let args = |files: Vec<ProjectFile>| DetectArgs {
        sheets: vec![Sheet {
            url: "http://fixture/page#media".to_owned(),
            text: text.clone(),
            media: None,
        }],
        files,
        merge_tolerance_px: Some(8),
        ..DetectArgs::default()
    };
    let labels = |files: Vec<ProjectFile>| -> Vec<(u32, Option<String>)> {
        detect(&args(files))
            .0
            .detected
            .iter()
            .map(|found| (found.width, found.label.clone()))
            .collect()
    };
    let named = |list: &[(u32, Option<&str>)]| -> Vec<(u32, Option<String>)> {
        list.iter()
            .map(|(width, label)| (*width, label.map(str::to_owned)))
            .collect()
    };
    assert_eq!(
        labels(Vec::new()),
        named(&[
            (768, Some("md")),
            (1024, Some("lg")),
            (1280, Some("xl")),
            (1600, None),
            (1920, None),
            (2000, None),
        ])
    );
    let config = "export default defineConfig<Theme>({ presets: [presetWind4()], theme: {\n\
        breakpoint: { sm: '768px', md: '1024px', lg: '1280px', xl: '1600px',\n\
        '2xl': '1920px', '3xl': '2000px', ultra: '2560px' } } })";
    assert_eq!(
        labels(vec![ProjectFile {
            path: "uno.config.ts".to_owned(),
            text: config.to_owned(),
        }]),
        named(&[
            (768, Some("sm")),
            (1024, Some("md")),
            (1280, Some("lg")),
            (1600, Some("xl")),
            (1920, Some("2xl")),
            (2000, Some("3xl")),
            (2560, Some("ultra")),
        ])
    );
}
