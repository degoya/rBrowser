use clap::Parser as _;
use rb_visual::sitemap::{Outcome as ShotOutcome, RunBreakpoint, Task};
use serde_json::json;

use super::*;
use crate::cli::Command;

fn parse(args: &[&str]) -> SitemapCliArgs {
    let cli = Cli::try_parse_from(args).expect("parses");
    match cli.command {
        Some(Command::Capture(CaptureCliArgs {
            command: CaptureCommand::Sitemap(args),
        })) => args,
        other => panic!("not capture sitemap: {other:?}"),
    }
}

#[test]
fn the_command_line_becomes_the_tools_arguments() {
    let args = parse(&[
        "rbrowser",
        "capture",
        "sitemap",
        "https://www.example.test/",
        "--breakpoints",
        "mobile,desktop",
        "--env",
        "local",
        "--max",
        "20",
        "--compare",
        "--json",
    ]);
    assert!(args.json);
    let capture = args.capture_args().expect("in bounds");
    assert_eq!(capture.url.as_deref(), Some("https://www.example.test/"));
    assert_eq!(capture.breakpoints, ["mobile", "desktop"]);
    assert_eq!(capture.env.as_deref(), Some("local"));
    assert_eq!(capture.max, Some(20));
    assert!(capture.compare);
    assert_eq!(capture.include, None);
    let bare = parse(&["rbrowser", "capture", "sitemap"]);
    assert_eq!(bare.capture_args().expect("ok"), CaptureArgs::default());
}

#[test]
fn visible_takes_only_the_views_the_grid_shows() {
    let args = parse(&["rbrowser", "capture", "sitemap", "--visible"]);
    let capture = args.capture_args().expect("in bounds");
    assert_eq!(
        capture.views,
        Some(crate::sitemap::tool::CaptureViews::Visible)
    );
    assert!(
        Cli::try_parse_from([
            "rbrowser",
            "capture",
            "sitemap",
            "--visible",
            "--breakpoints",
            "mobile"
        ])
        .is_err()
    );
    let both = SitemapCliArgs {
        visible: true,
        breakpoints: vec!["mobile".to_owned()],
        ..SitemapCliArgs::default()
    };
    assert!(both.capture_args().is_err());
}

#[test]
fn bookmarks_and_a_url_exclude_each_other() {
    assert!(
        Cli::try_parse_from(["rbrowser", "capture", "sitemap", "a.xml", "--bookmarks"]).is_err()
    );
    let too_many = SitemapCliArgs {
        max: Some(crate::sitemap::tool::MAX_PAGES + 1),
        ..SitemapCliArgs::default()
    };
    assert!(too_many.capture_args().is_err());
}

fn run(state: RunState, failed: bool) -> SitemapRun {
    let breakpoints = vec![RunBreakpoint {
        name: "mobile".to_owned(),
        label: "Mobile".to_owned(),
        width: 375,
        height: 800,
        dpr: 1.0,
    }];
    let pages = [("http://a.test/".to_owned(), None)];
    let mut run = SitemapRun::new("r1", "http://a.test/", breakpoints, &pages, 0);
    let task = Task {
        page: 0,
        breakpoint: "mobile".to_owned(),
    };
    let result = if failed {
        Err("timeout".to_owned())
    } else {
        Ok("index/mobile.png".to_owned())
    };
    run.record(
        &task,
        ShotOutcome {
            status: Some(200),
            final_url: None,
            result,
            ms: 1000,
        },
    );
    run.state = state;
    run
}

#[test]
fn the_exit_code_is_zero_only_for_a_finished_run_without_failures() {
    assert_eq!(exit_code(&run(RunState::Finished, false)), exit::OK);
    assert_eq!(exit_code(&run(RunState::Finished, true)), exit::FAILED);
    assert_eq!(exit_code(&run(RunState::Cancelled, false)), exit::FAILED);
    assert_eq!(
        progress_line(&run(RunState::Running, false)),
        "sitemap r1: 1/1 done, 0 failed, about 0 s left"
    );
}

#[test]
fn the_hand_over_maps_to_exit_codes() {
    let root = Path::new("/work/site");
    let answer = json!({ "runId": "r1", "folder": "f", "pending": 4 });
    assert_eq!(
        started(Outcome::Answered(answer), root)
            .expect("started")
            .run_id,
        "r1"
    );
    assert_eq!(
        started(Outcome::NotRunning, root).expect_err("none").0,
        exit::NO_INSTANCE
    );
    let refused = started(
        Outcome::Refused(Refusal::Failed, Some("sitemap.error.busy".to_owned())),
        root,
    )
    .expect_err("busy");
    assert_eq!(refused, (exit::FAILED, "sitemap.error.busy".to_owned()));
    assert_eq!(
        started(Outcome::Refused(Refusal::BadNonce, None), root)
            .expect_err("refused")
            .0,
        exit::HANDOVER_FAILED
    );
}
