use std::time::Duration;

use super::*;

#[test]
fn a_short_wait_is_not_logged_and_a_long_one_is() {
    assert_eq!(verdict(Duration::from_millis(20)), None);
    assert_eq!(verdict(SLOW - Duration::from_millis(1)), None);
    assert!(verdict(SLOW).is_some());
    assert!(verdict(Duration::from_secs(3)).is_some());
}
