use super::*;

fn breakpoints() -> Vec<RunBreakpoint> {
    ["Mobile S", "tablet", "desktop", "wide"]
        .iter()
        .map(|name| RunBreakpoint {
            name: (*name).to_owned(),
            label: (*name).to_owned(),
            width: 100,
            height: 100,
            dpr: 1.0,
        })
        .collect()
}

fn pages(count: usize) -> Vec<(String, Option<String>)> {
    (0..count)
        .map(|n| (format!("https://a.test/page-{n}"), None))
        .collect()
}

fn done(file: &str, ms: u64) -> Outcome {
    Outcome {
        status: Some(200),
        final_url: None,
        result: Ok(file.to_owned()),
        ms,
    }
}

#[test]
fn a_new_run_has_every_shot_pending_with_file_names_and_unique_slugs() {
    let pages = vec![
        ("https://a.test/".to_owned(), None),
        ("https://b.test/".to_owned(), None),
        ("https://a.test/con".to_owned(), None),
        (
            "https://a.test/x?y=1".to_owned(),
            Some("https://prod.test/x?y=1".to_owned()),
        ),
    ];
    let mut bps = breakpoints();
    bps.push(RunBreakpoint {
        name: "tablet".to_owned(),
        label: "duplicate".to_owned(),
        width: 1,
        height: 1,
        dpr: 1.0,
    });
    let run = SitemapRun::new("2026-11-11_00-00-00", "https://a.test/", bps, &pages, 7);
    let names: Vec<&str> = run.breakpoints.iter().map(|bp| bp.name.as_str()).collect();
    assert_eq!(names, ["Mobile-S", "tablet", "desktop", "wide"]);
    let slugs: Vec<&str> = run.pages.iter().map(|page| page.slug.as_str()).collect();
    assert_eq!(slugs, ["index", "index-2", "_con", "x-y-1"]);
    assert_eq!(run.tasks(false, 3).len(), 16);
    assert_eq!(run.version, RUN_VERSION);
    assert_eq!(run.created_unix, 7);
    assert_eq!(run.state, RunState::Running);
}

#[test]
fn file_names_are_plain() {
    assert_eq!(file_name("Desktop XL"), "Desktop-XL");
    assert_eq!(file_name("../../etc"), "etc");
    assert_eq!(file_name("lpt1"), "_lpt1");
    assert_eq!(file_name(""), "_");
    assert!(valid_name(&file_name(&"x".repeat(500))));
}

#[test]
fn record_takes_the_status_the_file_and_a_failure() {
    let mut run = SitemapRun::new("r", "s", breakpoints(), &pages(2), 0);
    let task = Task {
        page: 1,
        breakpoint: "tablet".to_owned(),
    };
    run.record(
        &task,
        Outcome {
            status: Some(404),
            final_url: Some("https://a.test/other".to_owned()),
            result: Ok("page-1/tablet.png".to_owned()),
            ms: 900,
        },
    );
    let page = &run.pages[1];
    assert_eq!(page.status, Some(404));
    assert_eq!(page.final_url.as_deref(), Some("https://a.test/other"));
    let shot = &page.shots["tablet"];
    assert_eq!(shot.state, ShotState::Done);
    assert_eq!(shot.file.as_deref(), Some("page-1/tablet.png"));
    assert_eq!(shot.attempts, 1);
    let wide = Task {
        page: 0,
        breakpoint: "wide".to_owned(),
    };
    run.record(
        &wide,
        Outcome {
            status: None,
            final_url: None,
            result: Err("timeout".to_owned()),
            ms: 30_000,
        },
    );
    let shot = &run.pages[0].shots["wide"];
    assert_eq!(shot.state, ShotState::Failed);
    assert_eq!(shot.error.as_deref(), Some("timeout"));
    run.record(
        &Task {
            page: 99,
            breakpoint: "wide".to_owned(),
        },
        done("x", 1),
    );
}

#[test]
fn tasks_skip_what_is_done_and_retry_failures_up_to_the_attempts() {
    let mut run = SitemapRun::new("r", "s", breakpoints(), &pages(2), 0);
    let tasks = run.tasks(false, 3);
    run.record(&tasks[0], done("a.png", 10));
    run.record(
        &tasks[1],
        Outcome {
            status: None,
            final_url: None,
            result: Err("timeout".to_owned()),
            ms: 10,
        },
    );
    assert_eq!(
        run.tasks(false, 3).len(),
        6,
        "done and failed are not pending"
    );
    assert_eq!(run.tasks(true, 3).len(), 7, "the failure is retried");
    for _ in 0..2 {
        run.record(
            &tasks[1],
            Outcome {
                status: None,
                final_url: None,
                result: Err("timeout".to_owned()),
                ms: 10,
            },
        );
    }
    assert_eq!(run.tasks(true, 3).len(), 6, "three attempts are enough");
}

#[test]
fn an_aborted_run_resumes_without_a_shot_twice() {
    let mut run = SitemapRun::new("r", "s", breakpoints(), &pages(5), 0);
    let mut taken: Vec<Task> = Vec::new();
    let tasks = run.tasks(true, 3);
    let half = tasks.len() / 2;
    for task in &tasks[..half] {
        run.record(task, done("x.png", 100));
        taken.push(task.clone());
    }
    run.state = RunState::Cancelled;
    let text = serde_json::to_string(&run).expect("serialises");
    let mut resumed: SitemapRun = serde_json::from_str(&text).expect("reads back");
    assert_eq!(resumed, run);
    for task in resumed.tasks(true, 3) {
        assert!(!taken.contains(&task), "{task:?} again");
        resumed.record(&task, done("y.png", 100));
        taken.push(task);
    }
    assert_eq!(taken.len(), 20);
    assert!(resumed.tasks(true, 3).is_empty());
    let progress = resumed.progress(4);
    assert_eq!(
        (progress.done, progress.pending, progress.total),
        (20, 0, 20)
    );
}

#[test]
fn progress_counts_and_estimates_the_rest() {
    let mut run = SitemapRun::new("r", "s", breakpoints(), &pages(5), 0);
    assert_eq!(run.progress(4).eta_seconds, None);
    for task in run.tasks(false, 3).into_iter().take(4) {
        run.record(&task, done("x.png", 2000));
    }
    let progress = run.progress(4);
    assert_eq!(progress.pages, 5);
    assert_eq!(progress.breakpoints, 4);
    assert_eq!(progress.total, 20);
    assert_eq!(progress.done, 4);
    assert_eq!(progress.pending, 16);
    assert_eq!(progress.eta_seconds, Some(8), "16 shots of 2 s on 4 lanes");
}

#[test]
fn run_json_uses_the_contracts_field_names() {
    let mut run = SitemapRun::new("r", "s", breakpoints(), &pages(1), 0);
    let task = run.tasks(false, 3)[0].clone();
    run.record(&task, done("page-0/Mobile-S.png", 5));
    let value = serde_json::to_value(&run).expect("serialises");
    assert_eq!(value["runId"], "r");
    assert_eq!(value["fullPage"], false);
    assert_eq!(value["state"], "running");
    assert_eq!(value["pages"][0]["slug"], "page-0");
    assert_eq!(value["pages"][0]["shots"]["Mobile-S"]["state"], "done");
    assert_eq!(
        value["pages"][0]["shots"]["Mobile-S"]["file"],
        "page-0/Mobile-S.png"
    );
    assert_eq!(value["pages"][0]["shots"]["tablet"]["state"], "pending");
}
