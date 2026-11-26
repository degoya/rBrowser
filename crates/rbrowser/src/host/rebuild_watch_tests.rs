use std::time::{Duration, Instant};

use super::Watch;

#[test]
fn a_loop_is_told_with_its_causes_counted_most_frequent_first() {
    let mut watch = Watch::new();
    let start = Instant::now();
    let mut told = None;
    for step in 0..11u64 {
        let cause = if step % 4 == 0 {
            "rebuild from host/view_host.rs:130"
        } else {
            "footer of view 3: 22 -> 44 DIP"
        };
        told = watch.note(start + Duration::from_millis(step * 30), cause.to_owned());
    }
    assert_eq!(
        told.as_deref(),
        Some("8x footer of view 3: 22 -> 44 DIP; 3x rebuild from host/view_host.rs:130")
    );
}

#[test]
fn a_rare_rebuild_is_not_told_and_old_causes_drop_out() {
    let mut watch = Watch::new();
    let start = Instant::now();
    for step in 0..10u64 {
        assert_eq!(
            watch.note(start + Duration::from_millis(step * 10), "old".to_owned()),
            None
        );
    }
    // Two seconds later the old ones are out of the window: eleven new ones are the loop.
    let later = start + Duration::from_secs(2);
    let mut told = None;
    for step in 0..11u64 {
        told = watch.note(later + Duration::from_millis(step * 10), "new".to_owned());
    }
    assert_eq!(told.as_deref(), Some("11x new"));
}
