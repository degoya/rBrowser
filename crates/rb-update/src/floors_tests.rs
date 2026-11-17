use super::*;

#[test]
fn a_floor_only_rises() {
    let mut floors = Floors::default();
    floors.raise(Channel::Stable, 5);
    floors.raise(Channel::Stable, 3);
    assert_eq!(floors.get(Channel::Stable), Some(5));
    assert_eq!(floors.get(Channel::Beta), None);
}

#[test]
fn floors_round_trip_and_a_broken_file_is_a_first_contact() {
    let data = tempfile::tempdir().expect("tempdir");
    assert_eq!(Floors::load(data.path()), Floors::default());
    let mut floors = Floors::default();
    floors.raise(Channel::Beta, 9);
    floors.store(data.path()).expect("store");
    assert_eq!(Floors::load(data.path()), floors);
    fs::write(Floors::path(data.path()), "{").expect("write");
    assert_eq!(Floors::load(data.path()), Floors::default());
}
