use std::sync::Arc;
use std::time::{Duration, Instant};

use super::{FAILURE_WINDOW, MAX_CRASH_LIMIT, PluginHealth, PluginHealths};
use rb_plugin_types::codes;

#[test]
fn the_third_failure_in_five_minutes_suspends() {
    let health = PluginHealth::default();
    let start = Instant::now();
    assert!(!health.record_failure_at(start));
    assert!(!health.record_failure_at(start + Duration::from_secs(60)));
    assert!(health.check().is_ok());
    assert!(health.record_failure_at(start + Duration::from_secs(120)));
    assert!(health.is_suspended());
    assert_eq!(
        health.check().expect_err("suspended").code(),
        Some(codes::SUSPENDED)
    );
    health.resume();
    assert!(health.check().is_ok(), "resumed, it runs again");
    assert!(
        !health.record_failure_at(start + Duration::from_secs(130)),
        "and counts from zero"
    );
}

#[test]
fn failures_older_than_the_window_do_not_count() {
    let health = PluginHealth::new(2);
    let start = Instant::now();
    assert!(!health.record_failure_at(start));
    assert!(!health.record_failure_at(start + FAILURE_WINDOW + Duration::from_secs(1)));
    assert!(!health.is_suspended());
}

#[test]
fn the_limit_is_held_to_its_range() {
    let health = PluginHealth::new(0);
    assert!(health.record_failure_at(Instant::now()), "at least one");
    let health = PluginHealth::new(1000);
    let start = Instant::now();
    for second in 0..u64::from(MAX_CRASH_LIMIT) - 1 {
        assert!(!health.record_failure_at(start + Duration::from_secs(second)));
    }
    assert!(health.record_failure_at(start + Duration::from_secs(20)));
}

// RB-090-59: every plugin has a health of its own - its failures, its suspension and its running
// calls are never another plugin's.

#[test]
fn the_book_gives_one_health_per_plugin_and_the_same_one_again() {
    let book = PluginHealths::new(2);
    let lighthouse = book.of("lighthouse");
    assert!(Arc::ptr_eq(&lighthouse, &book.of("lighthouse")));
    assert!(!Arc::ptr_eq(&lighthouse, &book.of("rulers-guides")));
}

#[test]
fn one_plugin_suspended_leaves_the_others_running_and_resume_touches_only_it() {
    let book = PluginHealths::new(2);
    let rulers = book.of("rulers-guides");
    let lighthouse = book.of("lighthouse");
    let todo = book.of("todo");
    let start = Instant::now();
    assert!(!rulers.record_failure_at(start));
    assert!(rulers.record_failure_at(start + Duration::from_secs(1)));
    assert!(!todo.record_failure_at(start));
    assert!(todo.record_failure_at(start));
    assert!(rulers.is_suspended() && todo.is_suspended());
    assert!(lighthouse.check().is_ok(), "another plugin keeps answering");
    assert_eq!(book.suspended(), ["rulers-guides", "todo"]);

    assert!(book.resume("rulers-guides"));
    assert!(rulers.check().is_ok());
    assert!(todo.is_suspended(), "resume touches only the one plugin");
    assert!(
        !book.resume("unknown"),
        "an id the book does not hold resumes nothing"
    );

    assert_eq!(book.resume_all(), ["todo"]);
    assert!(book.suspended().is_empty());
}

#[test]
fn the_crash_limit_follows_the_setting_for_every_plugin() {
    let book = PluginHealths::new(3);
    let held = book.of("a");
    book.set_crash_limit(1);
    assert!(
        held.record_failure_at(Instant::now()),
        "the held one follows"
    );
    assert!(
        book.of("b").record_failure_at(Instant::now()),
        "a later one starts with it"
    );
}

/// The starvation of diag14: lighthouse's `run` held a call slot for the whole run, and every
/// other plugin (`max_concurrent = 1`) waited for that slot until its page request was given up.
#[tokio::test(start_paused = true)]
async fn a_long_call_of_one_plugin_does_not_hold_another_plugins_slot() {
    let book = PluginHealths::default();
    let lighthouse = book.of("lighthouse");
    let rulers = book.of("rulers-guides");
    let _run = lighthouse.enter(4).await;
    assert!(lighthouse.busy(1));
    assert!(!rulers.busy(1), "the other plugin's slot is free");
    // A bound that gives up, not a wall-clock assertion: paused time, so the deadline only
    // passes when the enter waits.
    let entered = tokio::time::timeout(Duration::from_secs(30), rulers.enter(1)).await;
    assert!(
        entered.is_ok(),
        "the other plugin enters while the run goes on"
    );
}
