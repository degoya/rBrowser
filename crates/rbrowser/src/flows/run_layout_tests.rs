use rb_config::LayoutPreset;

use super::*;

fn selection(preset: Option<&str>, mode: LayoutMode, focus: Option<&str>) -> Selection {
    Selection {
        preset: preset.map(str::to_owned),
        mode,
        focus: focus.map(str::to_owned),
    }
}

fn config() -> ProjectConfig {
    let mut config = ProjectConfig::default();
    for (id, mode) in [
        ("review", LayoutMode::Grid),
        ("phone", LayoutMode::Focus),
        ("strip", LayoutMode::Horizontal),
    ] {
        config.layouts.presets.insert(
            id.to_owned(),
            LayoutPreset {
                mode,
                ..LayoutPreset::default()
            },
        );
    }
    config
}

#[test]
fn a_layout_that_hides_nothing_stays() {
    let focus = selection(None, LayoutMode::Focus, Some("w1600"));
    assert_eq!(wanted(&config(), &focus, "grid", false), None);
}

#[test]
fn focus_mode_gives_way_to_the_mode_asked_for_over_its_preset() {
    // RB-090-84: focus mode over the grid preset `review` shows the grid of `review`.
    let focus = selection(Some("review"), LayoutMode::Focus, Some("w1600"));
    assert_eq!(
        wanted(&config(), &focus, "grid", true),
        Some(selection(Some("review"), LayoutMode::Grid, None))
    );
    assert_eq!(
        wanted(&config(), &focus, "horizontal", true),
        Some(selection(Some("review"), LayoutMode::Horizontal, None))
    );
    // A focus preset is left for the top-level layout.
    let phone = selection(Some("phone"), LayoutMode::Focus, None);
    assert_eq!(
        wanted(&config(), &phone, "split", true),
        Some(selection(None, LayoutMode::Split, None))
    );
}

#[test]
fn a_preset_is_shown_in_its_own_mode_and_an_unknown_one_is_the_grid() {
    let focus = selection(None, LayoutMode::Focus, Some("w1600"));
    assert_eq!(
        wanted(&config(), &focus, "strip", true),
        Some(selection(Some("strip"), LayoutMode::Horizontal, None))
    );
    // A focus preset would hide the views again: the grid instead.
    for asked in ["phone", "no-such-preset"] {
        assert_eq!(
            wanted(&config(), &focus, asked, true),
            Some(selection(None, LayoutMode::Grid, None)),
            "{asked}"
        );
    }
}

#[test]
fn the_layout_shown_already_is_not_shown_again() {
    // Horizontal mode scrolled views out of sight; the run asks for horizontal mode.
    let strip = selection(None, LayoutMode::Horizontal, None);
    assert_eq!(wanted(&config(), &strip, "horizontal", true), None);
}

fn replaced(before: Selection, shown: Selection) -> Replaced {
    Replaced {
        before,
        shown,
        focus_return: Some(LayoutMode::Grid),
    }
}

#[test]
fn the_layout_comes_back_only_while_the_runs_layout_is_still_shown() {
    // Review of RB-090-84: a layout the person chose during the run stays.
    let focus = selection(None, LayoutMode::Focus, Some("w1600"));
    let grid = selection(None, LayoutMode::Grid, None);
    let run = replaced(focus.clone(), grid.clone());
    assert!(still_shown(&grid, &run));
    assert!(!still_shown(
        &selection(Some("strip"), LayoutMode::Horizontal, None),
        &run
    ));
    assert!(!still_shown(&focus, &run));
}

#[test]
fn a_run_all_batch_switches_once_and_shows_the_layout_before_at_its_end() {
    // Outside a batch a run restores its own layout.
    let first = replaced(
        selection(None, LayoutMode::Focus, Some("w1600")),
        selection(None, LayoutMode::Grid, None),
    );
    assert_eq!(keep_for_batch(first.clone()), Some(first.clone()));
    // In a batch the first switch is kept for its end, the runs restore nothing.
    batch_begin();
    assert_eq!(keep_for_batch(first.clone()), None);
    let later = replaced(
        selection(None, LayoutMode::Horizontal, None),
        selection(None, LayoutMode::Grid, None),
    );
    assert_eq!(keep_for_batch(later), None);
    assert_eq!(take_batch(), Some(first));
    // The batch is over: nothing is kept any more.
    assert_eq!(take_batch(), None);
}
