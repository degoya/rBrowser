use std::sync::{Arc, Mutex};

use super::lock;

#[test]
fn locks_and_gives_the_guard() {
    let mutex = Mutex::new(1);
    *lock(&mutex) += 1;
    assert_eq!(*lock(&mutex), 2);
}

#[test]
fn a_poisoned_mutex_still_locks_with_its_data() {
    let mutex = Arc::new(Mutex::new(vec![1]));
    let poisoner = Arc::clone(&mutex);
    let panicked = std::thread::spawn(move || {
        let _guard = lock(&poisoner);
        panic!("holder panics");
    })
    .join();
    assert!(panicked.is_err());
    assert!(mutex.is_poisoned());
    assert_eq!(*lock(&mutex), vec![1]);
}

#[test]
fn unsized_data_locks_too() {
    let boxed: Box<Mutex<[u8]>> = Box::new(Mutex::new([1, 2, 3]));
    assert_eq!(lock(&boxed).len(), 3);
}
