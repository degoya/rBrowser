use rb_visual::sitemap::RunBreakpoint;

use super::*;
use crate::compare::report::{RunEnvironment, RunPath};

fn run() -> EnvRun {
    let environment = |id: &str, url: &str| RunEnvironment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
    };
    let breakpoint = |name: &str| RunBreakpoint {
        name: name.to_owned(),
        label: name.to_owned(),
        width: 100,
        height: 100,
        dpr: 1.0,
    };
    let mut run = EnvRun {
        run_id: "2026-11-12_10-00-00".to_owned(),
        environments: vec![
            environment("local", "http://localhost:3000"),
            environment("staging<x>", "https://staging.test"),
        ],
        breakpoints: vec![breakpoint("mobile"), breakpoint("desktop")],
        paths: vec![RunPath {
            path: "/a?b=\"1\"".to_owned(),
            folder: "01-a".to_owned(),
        }],
        ..EnvRun::default()
    };
    run.fill(&[(0, 1)], crate::compare::plan::url_in);
    for (index, shot) in run.shots.iter_mut().enumerate() {
        shot.file = Some(format!("01-a/{index}.png"));
    }
    run.diffs[1].status = PairStatus::Differs;
    run.diffs[1].ratio = 0.25;
    run.diffs[1].image = Some("01-a/diff.png".to_owned());
    run.diffs[0].status = PairStatus::Same;
    run
}

#[test]
fn the_page_shows_every_pair_and_breakpoint_with_its_images_and_needs_no_network() {
    let page = render(&run(), &|key| format!("[{key}]"));
    assert!(page.contains("<td class=\"same\">"), "{page}");
    assert!(page.contains("<td class=\"differs\"><a href=\"#d0-0-1-1\">25.00 %</a> [differs]"));
    for index in 0..4 {
        assert!(
            page.contains(&format!("src=\"01-a/{index}.png\"")),
            "shot {index}"
        );
    }
    assert!(page.contains("src=\"01-a/diff.png\""));
    assert!(page.contains("[slider]") && page.contains("[side_by_side]"));
    assert!(page.contains("href=\"https://staging.test/a?b=&quot;1&quot;\""));
    assert!(!page.contains("staging<x>"), "values are escaped");
    for remote in ["http://", "https://"] {
        let loads = ["src=\"", "<link", "@import"]
            .iter()
            .any(|tag| page.contains(&format!("{tag}{remote}")));
        assert!(!loads, "nothing is loaded from {remote}");
    }
}
