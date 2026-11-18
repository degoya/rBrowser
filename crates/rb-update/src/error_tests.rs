use chrono::{DateTime, Utc};

use super::*;

fn at(seconds: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(seconds, 0).expect("time")
}

/// RB-090-14: an expired manifest is nothing to offer, never a refusal worth a notice; a replay
/// or a forgery is.
#[test]
fn an_expired_manifest_is_no_integrity_failure_and_a_replay_is() {
    let expired = UpdateError::Stale(StaleError::Expired {
        expired_at: at(1_790_000_000),
    });
    assert!(expired.is_expired());
    assert!(!expired.is_integrity());
    let replayed = UpdateError::Stale(StaleError::Replayed { saw: 1, known: 2 });
    assert!(replayed.is_integrity());
    assert!(!replayed.is_expired());
    let future = UpdateError::Stale(StaleError::FromTheFuture {
        issued_at: at(1_790_000_000),
    });
    assert!(future.is_integrity());
    assert!(UpdateError::BadSignature("forged".to_owned()).is_integrity());
    assert!(!UpdateError::NotPublished.is_integrity());
    assert!(!UpdateError::Fetch("offline".to_owned()).is_integrity());
}
