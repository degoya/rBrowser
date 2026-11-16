use std::path::Path;
use std::sync::Arc;

use clap::Parser;

use super::*;
use crate::capture::ShotAnswer;
use crate::cli::Cli;
use crate::instance::protocol::Refusal;
use crate::instance::server::{Handler, start};

#[derive(Debug, Parser)]
struct Probe {
    #[command(flatten)]
    args: ScreenshotCliArgs,
}

fn parse(line: &[&str]) -> Result<ScreenshotCliArgs, clap::Error> {
    let mut words = vec!["rbrowser"];
    words.extend_from_slice(line);
    Probe::try_parse_from(words).map(|probe| probe.args)
}

#[test]
fn the_flags_become_capture_arguments() {
    let args = parse(&["--views", "mobile,3", "--full", "--delay", "5", "--json"]).expect("valid");
    assert_eq!(args.views, vec!["mobile", "3"]);
    let capture = args.capture_args();
    assert_eq!(capture.mode, Some(CaptureMode::FullPage));
    assert_eq!(capture.delay_ms, Some(5000));
    assert_eq!(capture.target, Some(Target::All));
    assert!(args.json);
    let element = parse(&["--element", "#hero"])
        .expect("valid")
        .capture_args();
    assert_eq!(element.mode, Some(CaptureMode::Element));
    assert_eq!(element.selector.as_deref(), Some("#hero"));
    let overview = parse(&["--overview"]).expect("valid").capture_args();
    assert_eq!(overview.overview, Some(true));
    assert_eq!(overview.mode, Some(CaptureMode::Viewport));
}

#[test]
fn contradictions_and_long_delays_are_refused() {
    assert!(parse(&["--element", "a", "--full"]).is_err());
    assert!(parse(&["--element", "a", "--overview"]).is_err());
    assert!(parse(&["--delay", "61"]).is_err());
}

#[test]
fn views_are_chosen_by_breakpoint_or_id_in_layout_order() {
    let views = [
        (ViewId(1), Some("mobile".to_owned())),
        (ViewId(2), Some("desktop".to_owned())),
        (ViewId(3), None),
        (ViewId(4), Some("mobile".to_owned())),
    ];
    assert_eq!(select_views(&views, &[]), Ok(None));
    let wanted = ["3".to_owned(), "mobile".to_owned()];
    assert_eq!(
        select_views(&views, &wanted),
        Ok(Some(vec![ViewId(1), ViewId(3), ViewId(4)]))
    );
    assert!(select_views(&views, &["tablet".to_owned()]).is_err());
}

#[test]
fn the_output_is_paths_or_json() {
    let answer = ScreenshotAnswer {
        shots: vec![
            ShotAnswer {
                label: "Mobile".to_owned(),
                error: Some("not attached".to_owned()),
                ..ShotAnswer::default()
            },
            ShotAnswer {
                label: "Desktop".to_owned(),
                path: Some("/p/desktop.png".to_owned()),
                ..ShotAnswer::default()
            },
        ],
        overview: Some(ShotAnswer {
            label: "overview".to_owned(),
            path: Some("/p/overview.png".to_owned()),
            ..ShotAnswer::default()
        }),
        folder: Some("/p".to_owned()),
    };
    assert_eq!(
        render(&answer, false),
        "/p/desktop.png\n/p/overview.png\nerror: Mobile: not attached"
    );
    let json: serde_json::Value = serde_json::from_str(&render(&answer, true)).expect("JSON");
    assert_eq!(json["overview"]["path"], "/p/overview.png");
    assert_eq!(json["folder"], "/p");
}

fn env(cwd: &Path) -> Env {
    Env {
        cwd: cwd.to_path_buf(),
        windows: false,
        distro: None,
        recent: Vec::new(),
        open_last_project: true,
        focus_on_open: true,
    }
}

fn cli(line: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("rbrowser").chain(line.iter().copied())).expect("parses")
}

fn project(dir: &Path) -> PathBuf {
    let root = dir.join("site");
    std::fs::create_dir_all(root.join(rb_config::CONFIG_DIR)).expect("dir");
    std::fs::write(
        root.join(rb_config::CONFIG_DIR)
            .join(rb_config::CONFIG_FILE),
        "{\"version\":1}",
    )
    .expect("config");
    root
}

#[test]
fn the_request_names_the_views_and_an_absolute_folder() {
    let cwd = std::env::temp_dir().join("work");
    let args = parse(&["--views", " mobile ,3", "--out", "../shots", "--overview"]).expect("valid");
    let request = instance_request(&args, &env(&cwd)).expect("a request");
    let expected_out = std::env::temp_dir().join("shots");
    assert_eq!(
        request,
        Request::Screenshot {
            args: args.capture_args(),
            views: vec!["mobile".to_owned(), "3".to_owned()],
            out: Some(expected_out.to_str().expect("Unicode").to_owned()),
        }
    );
    let bare = parse(&[]).expect("valid");
    assert!(matches!(
        instance_request(&bare, &env(&cwd)),
        Ok(Request::Screenshot { ref views, out: None, .. }) if views.is_empty()
    ));
}

#[test]
fn a_selector_out_of_bounds_is_a_usage_error() {
    let cwd = std::env::temp_dir();
    let long = "a".repeat(crate::capture::cdp::MAX_SELECTOR_LEN + 1);
    let args = parse(&["--element", &long]).expect("valid");
    let (code, message) = instance_request(&args, &env(&cwd)).expect_err("refused");
    assert_eq!(code, exit::USAGE);
    assert!(message.contains("selector"), "{message}");
}

#[test]
fn the_project_is_given_or_found_and_urls_or_reload_do_not_fit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = project(dir.path());
    let root_text = root.to_string_lossy().into_owned();
    assert_eq!(
        project_root(
            &cli(&["screenshot", "--project", &root_text]),
            &env(dir.path())
        ),
        Ok(root.clone())
    );
    std::fs::create_dir_all(root.join("src")).expect("dir");
    assert_eq!(
        project_root(&cli(&["screenshot"]), &env(&root.join("src"))),
        Ok(root.clone())
    );
    let outside = tempfile::tempdir().expect("temp dir");
    let mut elsewhere = env(outside.path());
    elsewhere.recent = vec![root_text.clone()];
    let (code, _) = project_root(&cli(&["screenshot"]), &elsewhere).expect_err("no project");
    assert_eq!(code, exit::NO_PROJECT, "never the last project");
    let (code, _) = project_root(
        &cli(&["--reload", "screenshot", "--project", &root_text]),
        &env(dir.path()),
    )
    .expect_err("refused");
    assert_eq!(code, exit::USAGE);
    let (code, _) = project_root(
        &cli(&[
            "--url",
            "http://a.test/",
            "screenshot",
            "--project",
            &root_text,
        ]),
        &env(dir.path()),
    )
    .expect_err("refused");
    assert_eq!(code, exit::USAGE);
}

fn answer(error: Option<&str>) -> serde_json::Value {
    serde_json::to_value(ScreenshotAnswer {
        shots: vec![ShotAnswer {
            label: "Mobile".to_owned(),
            path: error.is_none().then(|| "/p/mobile.png".to_owned()),
            error: error.map(str::to_owned),
            ..ShotAnswer::default()
        }],
        overview: None,
        folder: Some("/p".to_owned()),
    })
    .expect("serializes")
}

#[test]
fn each_outcome_ends_with_its_exit_code() {
    let root = Path::new("/site");
    let done = ending(Outcome::Answered(answer(None)), root, false);
    assert_eq!(done.code, exit::OK);
    assert_eq!(done.stdout.as_deref(), Some("/p/mobile.png"));
    assert_eq!(done.stderr, None);
    let json = ending(Outcome::Answered(answer(None)), root, true);
    let printed: serde_json::Value =
        serde_json::from_str(json.stdout.as_deref().expect("printed")).expect("JSON");
    assert_eq!(printed["shots"][0]["path"], "/p/mobile.png");
    let partly = ending(Outcome::Answered(answer(Some("not attached"))), root, false);
    assert_eq!(partly.code, exit::FAILED);
    assert_eq!(
        partly.stdout.as_deref(),
        Some("error: Mobile: not attached")
    );
    for (outcome, code) in [
        (Outcome::NotRunning, exit::NO_INSTANCE),
        (
            Outcome::Refused(Refusal::Failed, Some("no view shows tablet".to_owned())),
            exit::FAILED,
        ),
        (
            Outcome::Refused(Refusal::BadNonce, None),
            exit::HANDOVER_FAILED,
        ),
        (
            Outcome::Refused(Refusal::BadRequest, None),
            exit::HANDOVER_FAILED,
        ),
        (Outcome::Failed("late".to_owned()), exit::HANDOVER_FAILED),
        (Outcome::Handed, exit::FAILED),
        (Outcome::Answered(serde_json::json!(1)), exit::FAILED),
    ] {
        let ended = ending(outcome, root, false);
        assert_eq!(ended.code, code, "{ended:?}");
        assert!(ended.stderr.is_some(), "{ended:?}");
    }
    let refused = ending(
        Outcome::Refused(Refusal::Failed, Some("no view shows tablet".to_owned())),
        root,
        false,
    );
    assert!(
        refused
            .stderr
            .is_some_and(|text| text.contains("no view shows tablet"))
    );
}

#[test]
fn the_running_instance_captures_and_the_call_ends_with_its_code() {
    let local = tempfile::tempdir().expect("temp dir");
    let root = project(local.path());
    let root_text = root.to_string_lossy().into_owned();
    let handler: Handler = Arc::new(|request: Request| match request {
        Request::Screenshot { views, .. } if views.is_empty() => Ok(Some(answer(None))),
        _ => Ok(Some(answer(Some("not attached")))),
    });
    let line = cli(&["screenshot", "--project", &root_text, "--json"]);
    let Some(crate::cli::Command::Screenshot(args)) = &line.command else {
        panic!("a screenshot command");
    };
    assert_eq!(
        run(&line, args, &env(local.path()), Some(local.path())),
        exit::NO_INSTANCE
    );
    assert_eq!(
        run(&line, args, &env(local.path()), None),
        exit::NO_INSTANCE
    );
    start(local.path(), &root, handler).expect("listens");
    assert_eq!(
        run(&line, args, &env(local.path()), Some(local.path())),
        exit::OK
    );
    let named = cli(&["screenshot", "--project", &root_text, "--views", "mobile"]);
    let Some(crate::cli::Command::Screenshot(args)) = &named.command else {
        panic!("a screenshot command");
    };
    assert_eq!(
        run(&named, args, &env(local.path()), Some(local.path())),
        exit::FAILED
    );
}
