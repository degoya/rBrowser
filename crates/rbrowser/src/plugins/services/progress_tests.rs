use std::time::{Duration, Instant};

use rb_ipc::PluginProgress;

use super::{Described, ProgressBoard, THROTTLE, describe_parts};

fn named(plugin: &str) -> Described {
    Described {
        name: format!("{plugin}!"),
        cancel: (plugin == "width-sweep").then(|| "plugin.width_sweep.cancel".to_owned()),
    }
}

fn entry(plugin: &str, done: u32, total: u32) -> PluginProgress {
    let described = named(plugin);
    PluginProgress {
        plugin: plugin.to_owned(),
        name: described.name,
        done,
        total,
        cancel: described.cancel,
    }
}

#[test]
fn the_first_report_of_a_call_is_told_at_once() {
    let board = ProgressBoard::default();
    let now = Instant::now();
    board.started("width-sweep");
    assert!(board.list(named).is_empty(), "no entry before a report");
    assert!(board.report("width-sweep", 1, 160, now));
    assert_eq!(board.list(named), [entry("width-sweep", 1, 160)]);
}

#[test]
fn reports_closer_than_the_throttle_are_kept_but_not_told() {
    let board = ProgressBoard::default();
    let start = Instant::now();
    board.started("width-sweep");
    assert!(board.report("width-sweep", 1, 160, start));
    let soon = start + THROTTLE / 2;
    assert!(!board.report("width-sweep", 2, 160, soon), "too soon");
    // The step is kept: the next told one, or the list read for another reason, has it.
    assert_eq!(board.list(named), [entry("width-sweep", 2, 160)]);
    assert!(board.report("width-sweep", 3, 160, start + THROTTLE));
    assert!(
        !board.report(
            "width-sweep",
            4,
            160,
            start + THROTTLE + Duration::from_millis(1)
        ),
        "the throttle counts from the last report told"
    );
}

#[test]
fn the_last_step_is_told_at_once() {
    let board = ProgressBoard::default();
    let start = Instant::now();
    board.started("width-sweep");
    assert!(board.report("width-sweep", 159, 160, start));
    assert!(board.report("width-sweep", 160, 160, start));
    assert_eq!(board.list(named), [entry("width-sweep", 160, 160)]);
}

#[test]
fn a_total_of_zero_is_throttled_like_any_other() {
    let board = ProgressBoard::default();
    let start = Instant::now();
    board.started("seo");
    assert!(board.report("seo", 0, 0, start));
    assert!(!board.report("seo", 0, 0, start), "0 of 0 is not the end");
}

#[test]
fn the_entry_goes_when_the_plugins_last_call_ends() {
    let board = ProgressBoard::default();
    let now = Instant::now();
    board.started("width-sweep");
    board.started("width-sweep");
    assert!(board.report("width-sweep", 5, 160, now));
    // The cancel call beside the sweep ends first: the sweep's entry stays.
    assert!(!board.ended("width-sweep"));
    assert_eq!(board.list(named).len(), 1);
    assert!(board.ended("width-sweep"), "the last call: the entry goes");
    assert!(board.list(named).is_empty());
    assert!(!board.ended("width-sweep"), "nothing left to end");
}

#[test]
fn a_call_that_reported_nothing_ends_without_a_change() {
    let board = ProgressBoard::default();
    board.started("seo");
    assert!(!board.ended("seo"), "no entry, nothing for the bar");
}

#[test]
fn a_report_outside_a_call_is_ignored() {
    let board = ProgressBoard::default();
    assert!(!board.report("width-sweep", 1, 2, Instant::now()));
    assert!(board.list(named).is_empty());
}

#[test]
fn a_new_call_after_the_end_starts_a_new_entry() {
    let board = ProgressBoard::default();
    let now = Instant::now();
    board.started("width-sweep");
    assert!(board.report("width-sweep", 160, 160, now));
    assert!(board.ended("width-sweep"));
    board.started("width-sweep");
    assert!(
        board.report("width-sweep", 1, 40, now),
        "the first report of the next call is told at once"
    );
    assert_eq!(board.list(named), [entry("width-sweep", 1, 40)]);
}

#[test]
fn several_plugins_are_listed_by_id() {
    let board = ProgressBoard::default();
    let now = Instant::now();
    for plugin in ["width-sweep", "seo"] {
        board.started(plugin);
        assert!(board.report(plugin, 1, 4, now));
    }
    assert_eq!(
        board.list(named),
        [entry("seo", 1, 4), entry("width-sweep", 1, 4)]
    );
}

#[test]
fn done_past_total_is_held_to_it() {
    let board = ProgressBoard::default();
    board.started("seo");
    assert!(board.report("seo", 9, 4, Instant::now()));
    assert_eq!(board.list(named), [entry("seo", 4, 4)]);
}

#[test]
fn a_feature_plugin_with_a_cancel_command_names_it() {
    assert_eq!(
        describe_parts("width-sweep", "Width sweep", true, &["run", "cancel"]),
        Described {
            name: "Width sweep".to_owned(),
            cancel: Some("plugin.width_sweep.cancel".to_owned()),
        }
    );
}

#[test]
fn no_cancel_command_or_no_feature_plugin_names_none() {
    assert_eq!(
        describe_parts("seo", "SEO", true, &["run"]).cancel,
        None,
        "no cancel command"
    );
    assert_eq!(
        describe_parts("seo", "SEO", false, &["cancel"]).cancel,
        None,
        "a check plugin's commands are not registered"
    );
}
