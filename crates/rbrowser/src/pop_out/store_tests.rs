use super::*;

fn rect(x: i32, width: i32) -> ScreenRect {
    ScreenRect {
        x,
        y: 40,
        width,
        height: 700,
    }
}

#[test]
fn bounds_are_kept_per_breakpoint_and_read_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(STATE_FILE);
    let mut state = load(&path);
    assert_eq!(state.bounds("desktop"), None);
    state.remember("desktop", rect(100, 1200));
    state.remember("mobile", rect(1400, 393));
    save(&path, &state).expect("saved");
    let read = load(&path);
    assert_eq!(read.bounds("desktop"), Some(rect(100, 1200)));
    assert_eq!(read.bounds("mobile"), Some(rect(1400, 393)));
    assert_eq!(read.bounds("tablet"), None);
}

#[test]
fn a_missing_or_broken_file_means_nothing_remembered() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(STATE_FILE);
    assert_eq!(load(&path), PopOutState::default());
    std::fs::write(&path, "{ not json").expect("written");
    assert_eq!(load(&path), PopOutState::default());
}

#[test]
fn unusable_bounds_are_neither_kept_nor_used() {
    let mut state = PopOutState::default();
    // A minimised window reports nonsense.
    state.remember("desktop", rect(-32_000, 0));
    assert_eq!(state.bounds("desktop"), None);
    state.windows.insert(
        "tablet".to_owned(),
        WindowBounds {
            x: 0,
            y: 0,
            width: 100_000,
            height: 700,
        },
    );
    assert_eq!(state.bounds("tablet"), None);
}

#[test]
fn the_file_holds_a_bounded_number_of_breakpoints() {
    let mut state = PopOutState::default();
    for index in 0..(MAX_ENTRIES + 10) {
        state.remember(&format!("bp{index:03}"), rect(10, 800));
    }
    assert_eq!(state.windows.len(), MAX_ENTRIES);
    // The newest is always kept.
    assert!(state.bounds(&format!("bp{:03}", MAX_ENTRIES + 9)).is_some());
}

#[test]
fn windows_closed_at_once_all_keep_their_bounds() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(STATE_FILE);
    let closers: Vec<_> = (0..8)
        .map(|index| {
            let path = path.clone();
            std::thread::spawn(move || {
                remember_and_save(&path, &format!("bp{index}"), rect(10 + index, 800))
                    .expect("saved");
            })
        })
        .collect();
    for closer in closers {
        closer.join().expect("no panic");
    }
    let read = load(&path);
    for index in 0..8 {
        assert_eq!(
            read.bounds(&format!("bp{index}")),
            Some(rect(10 + index, 800)),
            "bp{index} was lost"
        );
    }
}

#[test]
fn remembered_bounds_are_read_from_the_file_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(STATE_FILE);
    let mut state = PopOutState::default();
    state.remember("desktop", rect(100, 1200));
    save(&path, &state).expect("saved");
    warm(&path);
    std::fs::remove_file(&path).expect("removed");
    assert_eq!(
        remembered(&path, "desktop"),
        Some(rect(100, 1200)),
        "served from memory, no file read in the command"
    );
    remember_and_save(&path, "mobile", rect(1400, 393)).expect("saved");
    let read = load(&path);
    assert_eq!(
        read.bounds("desktop"),
        Some(rect(100, 1200)),
        "kept with the new one"
    );
    assert_eq!(remembered(&path, "mobile"), Some(rect(1400, 393)));
}
