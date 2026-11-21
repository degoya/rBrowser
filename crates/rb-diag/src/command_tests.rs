use std::path::Path;

use clap::Parser;
use rb_commands::McpCoverage;
use serde_json::json;

use super::*;
use crate::bundle::Versions;
use crate::paths::DiagPaths;

fn input_for(root: &Path) -> Arc<dyn Fn() -> BundleInput + Send + Sync> {
    let root = root.to_path_buf();
    Arc::new(move || BundleInput {
        paths: DiagPaths::under(&root),
        versions: Versions::default(),
        config_effective: None,
        plugins: json!([]),
        perf: Value::Null,
        max_crash_dumps: 0,
    })
}

#[test]
fn the_definition_is_reachable_by_leader_and_mcp() {
    let def = command_def().expect("def");
    assert_eq!(def.id.as_str(), "diagnostics.create");
    assert_eq!(def.title_key, "commands.diagnostics.create");
    assert_eq!(def.group, Group::Settings);
    assert_eq!(def.mcp, McpCoverage::Covered("diagnostics".to_owned()));
    assert_eq!(def.default_bindings.len(), 1);
    assert!(matches!(
        &def.default_bindings[0],
        Binding::Leader(chord) if chord.keys == [Key::Char(','), Key::Char('D')]
    ));
}

#[derive(Debug, Parser)]
struct Cli {
    #[command(subcommand)]
    command: Sub,
}

#[derive(Debug, clap::Subcommand)]
enum Sub {
    Diagnostics(DiagnosticsArgs),
}

#[test]
fn the_cli_arguments_parse() {
    let Cli {
        command: Sub::Diagnostics(args),
    } = Cli::try_parse_from(["rbrowser", "diagnostics", "--out", "d.zip"]).expect("parse");
    assert_eq!(args.out, Some(PathBuf::from("d.zip")));
    let Cli {
        command: Sub::Diagnostics(args),
    } = Cli::try_parse_from(["rbrowser", "diagnostics"]).expect("parse");
    assert_eq!(args.out, None);
}

#[test]
fn the_cli_writes_to_out() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("d.zip");
    let args = DiagnosticsArgs {
        out: Some(out.clone()),
    };
    let report = run_cli(&args, &input_for(dir.path())()).expect("run");
    assert_eq!(report.path, out);
    assert!(out.is_file());
}

#[test]
fn the_cli_defaults_to_the_diagnostics_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let report = run_cli(&DiagnosticsArgs::default(), &input_for(dir.path())()).expect("run");
    assert_eq!(
        report.path.parent(),
        Some(dir.path().join("diagnostics").as_path())
    );
    assert!(report.path.is_file());
}

#[test]
fn the_cli_never_overwrites_a_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("d.zip");
    std::fs::write(&out, "keep").expect("write");
    let args = DiagnosticsArgs {
        out: Some(out.clone()),
    };
    assert!(run_cli(&args, &input_for(dir.path())()).is_err());
    assert_eq!(std::fs::read_to_string(&out).expect("read"), "keep");
}

#[test]
fn two_bundles_in_the_same_second_get_two_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = input_for(dir.path())();
    let first = run_cli(&DiagnosticsArgs::default(), &input).expect("first");
    let second = run_cli(&DiagnosticsArgs::default(), &input).expect("second");
    assert_ne!(first.path, second.path);
    assert!(first.path.is_file() && second.path.is_file());
}

#[test]
fn the_handler_writes_only_into_the_diagnostics_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let run = handler(input_for(dir.path()));
    let outside = dir.path().join("outside.zip");
    for out in [
        json!(outside),
        json!("../outside.zip"),
        json!("sub/h.zip"),
        json!("h.txt"),
    ] {
        assert!(
            matches!(
                run(json!({ "out": out })),
                Err(CommandError::InvalidArgs(_))
            ),
            "{out}"
        );
    }
    assert!(!outside.exists());
    let kept = dir.path().join("diagnostics").join("kept.zip");
    std::fs::create_dir_all(dir.path().join("diagnostics")).expect("mkdir");
    std::fs::write(&kept, "keep").expect("write");
    assert!(run(json!({ "out": "kept.zip" })).is_err());
    assert_eq!(std::fs::read_to_string(&kept).expect("read"), "keep");
}

#[test]
fn the_handler_answers_path_and_entries() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("diagnostics").join("h.zip");
    let answer = handler(input_for(dir.path()))(json!({ "out": "h.zip" })).expect("answer");
    assert_eq!(answer["path"], json!(out));
    assert!(
        answer["entries"]
            .as_array()
            .expect("entries")
            .contains(&json!("versions.json"))
    );
}

#[test]
fn the_handler_refuses_bad_arguments() {
    let dir = tempfile::tempdir().expect("temp dir");
    let run = handler(input_for(dir.path()));
    assert!(matches!(
        run(json!({ "out": 3 })),
        Err(CommandError::InvalidArgs(_))
    ));
    assert!(matches!(
        run(json!({ "out": "" })),
        Err(CommandError::InvalidArgs(_))
    ));
}
