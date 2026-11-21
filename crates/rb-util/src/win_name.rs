//! A path segment as Windows reads it (the review of 0.9.6): case folded, trailing dots and
//! spaces dropped (`.git.` and `.GIT ` are `.git`), and whether it is an 8.3 short name
//! (`GIT~1` may be `.git`, `RBROWS~1` `.rbrowser`). A guard that compares names to keep a folder
//! out compares [`folded`] forms and refuses a [`is_short_name`] segment, on every platform: a
//! project folder may be shared with Windows.

/// `segment` as Windows compares it: trailing dots and spaces dropped, ASCII lower case. A
/// segment of dots and spaces only folds to `""`.
#[must_use]
pub fn folded(segment: &str) -> String {
    segment.trim_end_matches(['.', ' ']).to_ascii_lowercase()
}

/// Whether `segment` may be an 8.3 short name: a `~` followed by a digit.
#[must_use]
pub fn is_short_name(segment: &str) -> bool {
    segment
        .split('~')
        .skip(1)
        .any(|after| after.starts_with(|c: char| c.is_ascii_digit()))
}

#[cfg(test)]
#[path = "win_name_tests.rs"]
mod tests;
