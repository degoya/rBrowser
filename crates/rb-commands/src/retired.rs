//! Command ids rBrowser no longer registers, each with the command that took its place
//! (RB-090-63). A retired id still runs - as its successor, so a script, a palette entry kept
//! elsewhere or an agent's `runCommand` by id goes on working - and a user's saved binding of
//! it is dropped without a notice at every start; the successor keeps its own bindings.
//!
//! - `canvas.zoom.fit` (RB-090-63): `layout.zoom_to_fit`, one zoom-to-fit in every layout mode.

/// The retired ids, each with its successor.
const RETIRED: [(&str, &str); 1] = [("canvas.zoom.fit", "layout.zoom_to_fit")];

/// The command that took the place of the retired `id`; `None` for an id that is not retired.
#[must_use]
pub fn successor(id: &str) -> Option<&'static str> {
    RETIRED
        .iter()
        .find(|(retired, _)| *retired == id)
        .map(|(_, successor)| *successor)
}

#[cfg(test)]
#[path = "retired_tests.rs"]
mod tests;
