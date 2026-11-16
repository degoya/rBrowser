//! The export of the font list (RB-070-13): `.rbrowser/out/fonts/fonts.json` (the inventory as
//! `fonts_list` answers it) and `.rbrowser/out/fonts/fonts.csv` (a row per family and view). Each
//! export replaces the last one. A cell a spreadsheet would take as a formula (`=`, `+`, `-`, `@`,
//! a tab or a carriage return first) is written behind a `'`: family names come from the page.

use crate::inventory::Family;

/// The files below the plugin's out folder.
pub const JSON_FILE: &str = "fonts.json";
pub const CSV_FILE: &str = "fonts.csv";

const COLUMNS: &[&str] = &[
    "view",
    "family",
    "provider",
    "host",
    "web_font",
    "formats",
    "variable",
    "faces",
    "file_kb",
    "elements",
    "weights",
    "sizes_px",
    "rendered",
    "rendered_as",
];

/// One CSV cell: quoted when it holds a comma, quote or line break; a formula start defused.
#[must_use]
pub fn cell(value: &str) -> String {
    let defused = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_owned()
    };
    if defused.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", defused.replace('"', "\"\""))
    } else {
        defused
    }
}

fn join<T: ToString>(list: &[T]) -> String {
    list.iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ")
}

/// The CSV text of `families`: a row per family and view it is used or declared in.
#[must_use]
pub fn csv(families: &[Family]) -> String {
    let mut out = COLUMNS.join(",");
    out.push_str("\r\n");
    for family in families {
        let mut views: Vec<u32> = family.usage.iter().map(|usage| usage.view).collect();
        for face in &family.faces {
            for state in &face.status {
                if !views.contains(&state.view) {
                    views.push(state.view);
                }
            }
        }
        views.sort_unstable();
        for view in views {
            let usage = family.usage.iter().find(|usage| usage.view == view);
            let rendered = family.rendered.iter().find(|entry| entry.view == view);
            let faces = family
                .faces
                .iter()
                .filter(|face| face.status.iter().any(|state| state.view == view))
                .count();
            let row = [
                view.to_string(),
                family.family.clone(),
                family.provider.clone(),
                family.host.clone(),
                family.web_font.to_string(),
                family.formats.join(" "),
                family.variable.to_string(),
                faces.to_string(),
                family.file_kb.map_or_else(String::new, |kb| kb.to_string()),
                usage.map_or(0, |usage| usage.elements).to_string(),
                usage.map_or_else(String::new, |usage| join(&usage.weights)),
                usage.map_or_else(String::new, |usage| join(&usage.sizes)),
                rendered.map_or_else(String::new, |entry| entry.state.clone()),
                rendered.map_or_else(String::new, |entry| entry.instead.join(" ")),
            ];
            let cells: Vec<String> = row.iter().map(|value| cell(value)).collect();
            out.push_str(&cells.join(","));
            out.push_str("\r\n");
        }
    }
    out
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
