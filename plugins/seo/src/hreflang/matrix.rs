//! Reciprocity: every page of the set that answers 200 has to link to every other such page.
//! The matrix holds for each pair whether the row's page declares the column's (in any of the
//! three sources); each missing pair is a finding on the page that lacks the link -
//! `hreflang.return_missing` when the other page links to it, else `hreflang.set_incomplete`.

use serde::Serialize;

use crate::hreflang::load::{Answer, Loaded};
use crate::rules::Sink;

/// One cell of the matrix: does the row's page link to the column's?
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Cell {
    /// The page itself.
    Own,
    Yes,
    Missing,
    /// One of the two pages did not answer 200; not compared.
    Unknown,
}

/// A link the set lacks: `from` does not declare `to`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Pair {
    pub from: String,
    pub to: String,
}

/// One page of the set as the report lists it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SetPage {
    pub url: String,
    /// The code the set gives it; empty when none points to it.
    pub code: String,
    /// The status, `3xx` of a redirect, or why it was not loaded.
    pub answer: String,
}

/// The set and its reciprocity matrix (`matrix[row][column]`, rows and columns in `pages` order).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SetReport {
    pub pages: Vec<SetPage>,
    pub matrix: Vec<Vec<Cell>>,
    pub missing: Vec<Pair>,
}

fn answer(page: &Loaded) -> String {
    match &page.answer {
        Answer::Status(status) | Answer::Redirect(status) => status.to_string(),
        Answer::Failed(code) => code.clone(),
        Answer::Skipped(reason) => format!("{reason:?}").to_ascii_lowercase(),
    }
}

/// The matrix of `pages` (each with its code), its missing pairs reported into `sink`.
pub fn build(pages: &[(Loaded, String)], sink: &mut Sink<'_>) -> SetReport {
    let mut report = SetReport {
        pages: pages
            .iter()
            .map(|(page, code)| SetPage {
                url: page.url.to_string(),
                code: code.clone(),
                answer: answer(page),
            })
            .collect(),
        ..SetReport::default()
    };
    for (row, (from, _)) in pages.iter().enumerate() {
        let mut cells = Vec::with_capacity(pages.len());
        for (column, (to, code)) in pages.iter().enumerate() {
            let cell = if row == column {
                Cell::Own
            } else if !from.ok() || !to.ok() {
                Cell::Unknown
            } else if from.declares(&to.url) {
                Cell::Yes
            } else {
                Cell::Missing
            };
            if cell == Cell::Missing {
                report.missing.push(Pair {
                    from: from.url.to_string(),
                    to: to.url.to_string(),
                });
                sink.page(&from.url.to_string(), from.view);
                let rule = if to.declares(&from.url) {
                    "hreflang.return_missing"
                } else {
                    "hreflang.set_incomplete"
                };
                sink.emit(
                    rule,
                    rule,
                    None,
                    &[("target", to.url.to_string()), ("code", code.clone())],
                );
            }
            cells.push(cell);
        }
        report.matrix.push(cells);
    }
    report
}

#[cfg(test)]
#[path = "matrix_tests.rs"]
mod tests;
