use rb_engine::ViewId;

use super::*;

const A: ViewId = ViewId(1);
const B: ViewId = ViewId(2);

#[test]
fn a_view_is_read_once_its_loads_are_quiet() {
    let mut due = Due::default();
    // A redirect chain: each commit restarts the wait, the last ticket counts.
    let first = due.navigated(A);
    let last = due.navigated(A);
    assert!(!due.take(A, first));
    assert!(due.take(A, last));
    // A wait is used once.
    assert!(!due.take(A, last));
}

#[test]
fn the_views_wait_each_for_itself() {
    // Views on different documents do not end each other's waits.
    let mut due = Due::default();
    let a = due.navigated(A);
    let b = due.navigated(B);
    assert!(due.take(A, a));
    assert!(due.take(B, b));
}

#[test]
fn a_load_that_ended_is_read_again() {
    // A reload commits no new address: the dev server that came up, the CSS that changed.
    let mut due = Due::default();
    let first = due.updated(A, 1).expect("the first load ended");
    assert!(due.take(A, first));
    // A title, a size: no load ended.
    assert_eq!(due.updated(A, 1), None);
    let again = due.updated(A, 2).expect("a reload ended");
    assert!(due.take(A, again));
    assert_eq!(due.updated(B, 0), None, "a view that has not loaded yet");
}

#[test]
fn a_closed_view_and_a_reset_end_the_waits_and_keep_the_count_of_loads() {
    let mut due = Due::default();
    assert!(due.updated(A, 3).is_some());
    let a = due.navigated(A);
    due.forget(A);
    assert!(!due.take(A, a));
    let (a, b) = (due.navigated(A), due.navigated(B));
    due.reset();
    assert!(!due.take(A, a));
    assert!(!due.take(B, b));
    // A title or a size afterwards is no ended load: no read.
    assert_eq!(due.updated(A, 3), None);
    assert!(due.updated(A, 4).is_some());
}

#[test]
fn the_loads_read_with_a_wait_start_no_second_one() {
    // The setting switched on with the pages long loaded: one read per view.
    let mut due = Due::default();
    due.seen(A, 5);
    let ticket = due.navigated(A);
    assert_eq!(due.updated(A, 5), None);
    assert!(due.take(A, ticket));
    assert!(due.updated(A, 6).is_some());
    // Seeing fewer loads than known forgets none.
    due.seen(A, 2);
    assert_eq!(due.updated(A, 6), None);
}

#[test]
fn only_a_change_of_the_switch_itself_counts() {
    let mut switch = Switch::default();
    // The start: nothing to do, the page loads come by themselves.
    assert_eq!(switch.set(true), None);
    // Any other setting changed (the footer shown or hidden, a field): nothing.
    assert_eq!(switch.set(true), None);
    assert_eq!(switch.set(false), Some(Change::Off));
    assert_eq!(switch.set(false), None);
    assert_eq!(switch.set(true), Some(Change::On));
    // Started switched off: nothing to drop.
    let mut off = Switch::default();
    assert_eq!(off.set(false), None);
    assert_eq!(off.set(true), Some(Change::On));
}

#[test]
fn a_document_is_analysed_by_one_view_at_a_time() {
    let mut reading = Reading::default();
    assert!(reading.start("https://a.test/", 7));
    // The other views of the grid read the same page at the same moment.
    assert!(!reading.start("https://a.test/", 7));
    // Another content of the same document is another analysis.
    assert!(reading.start("https://a.test/", 8));
    reading.end("https://a.test/", 7);
    assert!(reading.start("https://a.test/", 7));
}
