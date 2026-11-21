use rb_engine::{CellRect, ViewId};

use super::*;

fn at(view: u32, x: i32) -> PlacedView {
    PlacedView {
        view: ViewId(view),
        outer: CellRect {
            x,
            y: 60,
            width: 200,
            height: 420,
        },
        visible: true,
    }
}

// RB-094-18: every placement went to the UI thread as a task of its own; while the thread was
// busy the moves of a drag queued up and the window replayed each stale one after the other.
// A placement that runs after a newer one of the same view was posted moves nothing.
#[test]
fn a_placement_overtaken_by_a_newer_one_of_its_view_moves_nothing() {
    let mut tickets = PlaceTickets::default();
    let first = tickets.post(&[at(1, 10), at(2, 10)]);
    let second = tickets.post(&[at(1, 20)]);
    let third = tickets.post(&[at(1, 30)]);
    // The window runs them in the order they were posted.
    assert_eq!(
        tickets.take(first, vec![at(1, 10), at(2, 10)]),
        (vec![at(2, 10)], 1)
    );
    assert_eq!(tickets.take(second, vec![at(1, 20)]), (Vec::new(), 1));
    assert_eq!(tickets.take(third, vec![at(1, 30)]), (vec![at(1, 30)], 0));
}

#[test]
fn a_placement_run_is_forgotten_and_the_next_one_counts_again() {
    let mut tickets = PlaceTickets::default();
    let first = tickets.post(&[at(1, 10)]);
    assert_eq!(tickets.take(first, vec![at(1, 10)]).1, 0);
    assert!(tickets.newest.is_empty(), "nothing pending is kept");
    let second = tickets.post(&[at(1, 20)]);
    assert_ne!(first, second);
    assert_eq!(tickets.take(second, vec![at(1, 20)]), (vec![at(1, 20)], 0));
}
