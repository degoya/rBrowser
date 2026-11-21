use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Snapshots;

#[test]
fn nothing_before_the_first_put_then_each_put_in_effect() {
    let snapshots: Snapshots<Vec<u32>> = Snapshots::new();
    assert!(snapshots.get().is_none());
    assert_eq!(snapshots.generation(), 0);
    assert!(snapshots.put(Arc::new(vec![1])).is_none());
    assert_eq!(snapshots.generation(), 1);
    let old = snapshots.put(Arc::new(vec![2, 2])).expect("the first");
    assert_eq!(*old, vec![1]);
    assert_eq!(*snapshots.get().expect("the second"), vec![2, 2]);
    assert_eq!(snapshots.generation(), 2);
}

#[test]
fn a_snapshot_taken_before_a_swap_stays_whole() {
    // A call that took the plugins keeps them until it ends, whatever is swapped meanwhile.
    let snapshots: Snapshots<Vec<u32>> = Snapshots::new();
    snapshots.put(Arc::new(vec![1, 1, 1]));
    let held = snapshots.get().expect("put");
    snapshots.put(Arc::new(vec![2]));
    drop(snapshots.put(Arc::new(vec![3, 3])));
    assert_eq!(*held, vec![1, 1, 1]);
    assert_eq!(*snapshots.get().expect("put"), vec![3, 3]);
}

#[test]
fn readers_during_swaps_see_one_whole_snapshot_each() {
    // Every snapshot is `n` copies of `n`: a reader that saw a mix would see a torn swap.
    let snapshots: Arc<Snapshots<Vec<u32>>> = Arc::new(Snapshots::new());
    snapshots.put(Arc::new(vec![1]));
    let done = Arc::new(AtomicBool::new(false));
    let readers: Vec<_> = (0..4)
        .map(|_| {
            let (snapshots, done) = (Arc::clone(&snapshots), Arc::clone(&done));
            std::thread::spawn(move || {
                let mut seen = 0_u32;
                while !done.load(Ordering::Acquire) {
                    let snapshot = snapshots.get().expect("never empty again");
                    let first = snapshot[0];
                    assert!(snapshot.iter().all(|value| *value == first));
                    assert_eq!(snapshot.len(), first as usize);
                    // Generations only grow.
                    assert!(first >= seen);
                    seen = first;
                }
                seen
            })
        })
        .collect();
    for n in 2..=200_u32 {
        snapshots.put(Arc::new(vec![n; n as usize]));
    }
    done.store(true, Ordering::Release);
    for reader in readers {
        assert!(reader.join().expect("no reader panicked") <= 200);
    }
    assert_eq!(snapshots.generation(), 200);
    assert_eq!(snapshots.get().expect("put").len(), 200);
}
