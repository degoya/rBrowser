use super::*;

fn bounds(x: i32, y: i32, width: u32, height: u32) -> WindowBounds {
    WindowBounds {
        x,
        y,
        width,
        height,
    }
}

#[test]
fn a_missing_or_broken_file_gives_the_defaults() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(UI_STATE_FILE);
    assert_eq!(load(&path), UiState::default());
    std::fs::write(&path, "{ not json").expect("writes");
    assert_eq!(load(&path), UiState::default());
    std::fs::write(&path, r#"{ "window": { "x": "far" } }"#).expect("writes");
    assert_eq!(load(&path), UiState::default());
}

#[test]
fn an_older_file_with_the_dock_size_still_reads() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(UI_STATE_FILE);
    std::fs::write(
        &path,
        r#"{ "dock": { "bottom": 400 }, "window": { "x": 1, "y": 2, "width": 800, "height": 600 } }"#,
    )
    .expect("writes");
    assert_eq!(load(&path).window, Some(bounds(1, 2, 800, 600)));
}

#[test]
fn the_state_survives_a_round_trip_into_a_new_folder() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("rBrowser").join(UI_STATE_FILE);
    let state = UiState {
        window: Some(bounds(100, 80, 1200, 800)),
    };
    save(&path, &state).expect("saves");
    assert_eq!(load(&path), state);
    let text = std::fs::read_to_string(&path).expect("reads");
    assert!(text.contains("\"window\""), "{text}");
}

#[test]
fn only_usable_window_bounds_are_opened_at() {
    let mut state = UiState {
        window: Some(bounds(-8, -8, 1200, 800)),
    };
    assert_eq!(state.window_bounds(), Some(bounds(-8, -8, 1200, 800)));
    // A minimised window reports a tiny rectangle far off-screen.
    state.window = Some(bounds(-32_000, -32_000, 160, 28));
    assert_eq!(state.window_bounds(), None);
    state.window = Some(bounds(0, 0, 100_000, 800));
    assert_eq!(state.window_bounds(), None);
    state.window = None;
    assert_eq!(state.window_bounds(), None);
}
