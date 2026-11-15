use rb_visual::sitemap::{Outcome, RunBreakpoint, RunState, Task};

use super::*;

fn run() -> SitemapRun {
    let breakpoints = vec![RunBreakpoint {
        name: "mobile".to_owned(),
        label: "Mobile".to_owned(),
        width: 375,
        height: 800,
        dpr: 1.0,
    }];
    let pages = [
        ("http://a.test/".to_owned(), None),
        ("http://a.test/missing".to_owned(), None),
        ("http://a.test/moved".to_owned(), None),
        ("http://a.test/slow".to_owned(), None),
    ];
    let mut run = SitemapRun::new("r", "http://a.test/", breakpoints, &pages, 0);
    let outcome = |status, final_url: Option<&str>, result: Result<&str, &str>| Outcome {
        status,
        final_url: final_url.map(str::to_owned),
        result: result.map(str::to_owned).map_err(str::to_owned),
        ms: 10,
    };
    let task = |page| Task {
        page,
        breakpoint: "mobile".to_owned(),
    };
    run.record(&task(0), outcome(Some(200), None, Ok("index/mobile.png")));
    run.record(&task(1), outcome(Some(404), None, Ok("missing/mobile.png")));
    run.record(
        &task(2),
        outcome(Some(200), Some("http://a.test/new"), Ok("moved/mobile.png")),
    );
    run.record(&task(3), outcome(None, None, Err("timeout")));
    run
}

#[test]
fn the_problems_are_the_errors_redirects_and_failed_shots() {
    let problems = problems(&run());
    let urls: Vec<&str> = problems
        .iter()
        .map(|problem| problem.url.as_str())
        .collect();
    assert_eq!(
        urls,
        [
            "http://a.test/missing",
            "http://a.test/moved",
            "http://a.test/slow"
        ]
    );
    assert_eq!(problems[0].status, Some(404));
    assert_eq!(problems[1].final_url.as_deref(), Some("http://a.test/new"));
    assert_eq!(
        problems[2].failed,
        [("mobile".to_owned(), "timeout".to_owned())]
    );
}

#[test]
fn the_status_names_the_gallery_once_it_is_written() {
    let dir = tempfile::tempdir().expect("temp");
    let run = run();
    let answer = status_of(&run, dir.path(), 4);
    assert_eq!(answer.state, RunState::Running);
    assert_eq!(answer.progress.done, 3);
    assert_eq!(answer.progress.failed, 1);
    assert_eq!(answer.gallery, None);
    std::fs::write(dir.path().join(GALLERY_FILE), "<html>").expect("gallery");
    assert!(status_of(&run, dir.path(), 4).gallery.is_some());
}

#[test]
fn every_error_has_its_code() {
    assert_eq!(RunError::Busy.code(), "sitemap.error.busy");
    assert_eq!(
        RunError::Plan(PlanError::NoSource).code(),
        "sitemap.error.no_source"
    );
    assert!(
        RunError::NoPages
            .to_string()
            .starts_with("sitemap.error.no_pages")
    );
}
