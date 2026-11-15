use super::*;

#[test]
fn the_same_seed_gives_the_same_sequence() {
    let mut a = Rng::new(42);
    let mut b = Rng::new(42);
    let first: Vec<u64> = (0..8).map(|_| a.next_u64()).collect();
    let second: Vec<u64> = (0..8).map(|_| b.next_u64()).collect();
    assert_eq!(first, second);
    assert_ne!(Rng::new(43).next_u64(), first[0]);
}

#[test]
fn a_keyed_generator_depends_on_every_part_and_their_boundaries() {
    let one = Rng::keyed(7, &["valid", "email"]).next_u64();
    assert_eq!(one, Rng::keyed(7, &["valid", "email"]).next_u64());
    assert_ne!(one, Rng::keyed(8, &["valid", "email"]).next_u64());
    assert_ne!(one, Rng::keyed(7, &["invalid", "email"]).next_u64());
    assert_ne!(
        Rng::keyed(7, &["ab", "c"]).next_u64(),
        Rng::keyed(7, &["a", "bc"]).next_u64()
    );
}

#[test]
fn below_stays_below_and_zero_is_zero() {
    let mut rng = Rng::new(1);
    for n in [1_u64, 2, 3, 10, 1000] {
        for _ in 0..50 {
            assert!(rng.below(n) < n);
        }
    }
    assert_eq!(rng.below(0), 0);
}

#[test]
fn between_is_inclusive_and_handles_a_reversed_range() {
    let mut rng = Rng::new(9);
    for _ in 0..200 {
        let n = rng.between(-3, 3);
        assert!((-3..=3).contains(&n));
    }
    assert_eq!(rng.between(5, 5), 5);
    assert_eq!(rng.between(6, 2), 6);
}

#[test]
fn pick_takes_an_element_and_digits_are_digits() {
    let mut rng = Rng::new(3);
    let items = ["a", "b", "c"];
    for _ in 0..20 {
        assert!(items.contains(rng.pick(&items)));
    }
    let digits = rng.digits(12);
    assert_eq!(digits.len(), 12);
    assert!(digits.chars().all(|c| c.is_ascii_digit()));
}
