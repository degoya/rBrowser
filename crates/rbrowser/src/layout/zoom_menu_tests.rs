use serde_json::{Value, json};

use super::*;

fn checked(rows: &[ZoomRow]) -> Vec<&str> {
    rows.iter()
        .filter(|row| row.checked)
        .map(|row| row.label.as_str())
        .collect()
}

// RB-090-18: the presets were a `<select>` of the status bar page, whose list the page placed
// below the field - off the screen with the bar at its bottom. The core's native menu has the
// same rows, Fit first.
#[test]
fn the_rows_are_fit_then_the_presets_in_per_cent() {
    let rows = rows(Some(1.0));
    let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            rb_i18n::t(KEY_FIT).as_str(),
            "25 %",
            "50 %",
            "75 %",
            "100 %",
            "150 %",
            "200 %"
        ]
    );
    assert_eq!(rows[0].action, ZoomAction::Fit);
    assert_eq!(rows[1].action, ZoomAction::Level(0.25));
    assert_eq!(rows[6].action, ZoomAction::Level(2.0));
}

#[test]
fn the_current_level_has_the_check_and_a_level_between_the_presets_none() {
    assert_eq!(checked(&rows(Some(0.75))), ["75 %"]);
    // The canvas level is rounded to a tenth of a per cent (`canvas_level::current`).
    assert_eq!(checked(&rows(Some(1.4999))), ["150 %"]);
    assert!(checked(&rows(Some(0.066))).is_empty());
    assert!(checked(&rows(None)).is_empty());
    // Fit is a command, never a level.
    assert!(!rows(Some(1.0))[0].checked);
}

#[test]
fn a_row_runs_the_zoom_command_of_the_one_registry() {
    assert_eq!(command(ZoomAction::Fit), ("canvas.zoom.fit", Value::Null));
    assert_eq!(
        command(ZoomAction::Level(0.5)),
        ("canvas.zoom.reset", json!({ "level": 0.5 }))
    );
}
