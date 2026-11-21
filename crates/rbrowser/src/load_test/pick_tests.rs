use super::*;
use crate::load_test::scenario::{Order, Think};

#[test]
fn a_sequence_goes_round_in_order() {
    let mut pick = Picker::new(Order::Sequence, &[1, 1, 1], 7);
    let picked: Vec<usize> = (0..7).map(|_| pick.next_step()).collect();
    assert_eq!(picked, vec![0, 1, 2, 0, 1, 2, 0]);
}

#[test]
fn weights_set_how_often_a_step_comes() {
    let mut pick = Picker::new(Order::Weighted, &[3, 1], 42);
    let mut counts = [0_u32; 2];
    for _ in 0..4000 {
        counts[pick.next_step()] += 1;
    }
    // Three to one, give or take.
    assert!((2700..=3300).contains(&counts[0]), "{counts:?}");
    assert_eq!(counts[0] + counts[1], 4000);
}

#[test]
fn the_same_seed_picks_the_same_and_users_differ() {
    let picks = |seed| {
        let mut pick = Picker::new(Order::Weighted, &[1, 1, 1, 1], seed);
        (0..20).map(|_| pick.next_step()).collect::<Vec<_>>()
    };
    assert_eq!(picks(1), picks(1));
    assert_ne!(picks(1), picks(2));
}

#[test]
fn a_think_time_lies_in_its_range() {
    let mut pick = Picker::new(Order::Sequence, &[1], 3);
    assert_eq!(pick.think(Think::default()).as_millis(), 0);
    assert_eq!(pick.think(Think { min: 250, max: 250 }).as_millis(), 250);
    for _ in 0..200 {
        let ms = pick.think(Think { min: 100, max: 200 }).as_millis();
        assert!((100..=200).contains(&ms), "{ms}");
    }
}
