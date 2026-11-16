use super::*;

#[test]
fn by_default_only_the_leader_submits() {
    assert!(
        LeaderOnly
            .submitters(ViewId(1), &[ViewId(2), ViewId(3)])
            .is_empty()
    );
}

#[test]
fn a_follow_waits_for_the_leader_within_its_window() {
    let now = Instant::now();
    let follow = Follow::new(ViewId(1), vec![ViewId(2)], now);
    assert!(follow.is_due(ViewId(1), now + Duration::from_secs(1)));
    assert!(
        !follow.is_due(ViewId(2), now),
        "a follower's navigation is not it"
    );
    assert!(!follow.expired(now + FOLLOW_WINDOW));
    assert!(follow.expired(now + FOLLOW_WINDOW + Duration::from_millis(1)));
    assert!(!follow.is_due(ViewId(1), now + FOLLOW_WINDOW + Duration::from_millis(1)));
}
