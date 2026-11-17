use std::collections::BTreeSet;
use std::sync::Mutex;

use super::*;
use crate::cdp::lock;

/// The hub's exclusions; `refuse` is a view the hub does not know.
#[derive(Default)]
struct Switch {
    excluded: Mutex<BTreeSet<u32>>,
    calls: Mutex<Vec<(u32, bool)>>,
    refuse: Option<u32>,
}

impl SyncSwitch for Switch {
    fn is_excluded(&self, view: ViewId) -> bool {
        lock(&self.excluded).contains(&view.0)
    }

    fn exclude(&self, view: ViewId, excluded: bool) -> Result<(), String> {
        if self.refuse == Some(view.0) {
            return Err(format!("unknown view {view}"));
        }
        lock(&self.calls).push((view.0, excluded));
        let mut set = lock(&self.excluded);
        if excluded {
            set.insert(view.0);
        } else {
            set.remove(&view.0);
        }
        Ok(())
    }
}

fn views(ids: &[u32]) -> Vec<ViewId> {
    ids.iter().copied().map(ViewId).collect()
}

#[test]
fn the_views_of_a_run_leave_sync_and_come_back() {
    let switch = Arc::new(Switch::default());
    let pause = SyncPause::new(switch.clone(), &views(&[1, 2]));
    assert_eq!(*lock(&switch.excluded), BTreeSet::from([1, 2]));
    assert_eq!(pause.paused(), views(&[1, 2]));
    drop(pause);
    assert!(lock(&switch.excluded).is_empty());
}

#[test]
fn a_view_excluded_before_the_run_stays_excluded() {
    let switch = Arc::new(Switch::default());
    lock(&switch.excluded).insert(2);
    let pause = SyncPause::new(switch.clone(), &views(&[1, 2]));
    assert_eq!(pause.paused(), views(&[1]));
    drop(pause);
    assert_eq!(*lock(&switch.excluded), BTreeSet::from([2]));
    assert_eq!(*lock(&switch.calls), [(1, true), (1, false)]);
}

#[test]
fn a_view_the_hub_does_not_know_is_not_taken_back() {
    let switch = Arc::new(Switch {
        refuse: Some(2),
        ..Switch::default()
    });
    let pause = SyncPause::new(switch.clone(), &views(&[1, 2]));
    assert_eq!(pause.paused(), views(&[1]));
    drop(pause);
    assert_eq!(*lock(&switch.calls), [(1, true), (1, false)]);
}

#[test]
fn a_run_that_panics_still_gives_its_views_back() {
    let switch = Arc::new(Switch::default());
    let inner = switch.clone();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _pause = SyncPause::new(inner, &views(&[1, 3]));
        panic!("the run broke");
    }));
    assert!(outcome.is_err());
    assert!(lock(&switch.excluded).is_empty());
}
