//! The report's language set and parity sections (RB-060-09): the reciprocity matrix as a table
//! with row and column headers - every cell spelled out, never only a colour - and the list of
//! exactly the missing pairs; the views compared, the reference marked.

use std::fmt::Write as _;

use crate::hreflang::matrix::{Cell, SetReport};
use crate::i18n;
use crate::parity::ParityReport;
use crate::report::html::escape;

fn label(key: &str) -> String {
    escape(&i18n::text(&format!("plugin.seo.report.{key}"), &[]))
}

fn cell(cell: Cell) -> (&'static str, String) {
    match cell {
        Cell::Own => ("own", label("matrix.own")),
        Cell::Yes => ("yes", label("matrix.yes")),
        Cell::Missing => ("missing", label("matrix.missing")),
        Cell::Unknown => ("unknown", label("matrix.unknown")),
    }
}

/// The language set: its pages, the matrix, the missing pairs.
pub fn hreflang(out: &mut String, set: &SetReport) {
    let name = |index: usize| {
        let page = &set.pages[index];
        if page.code.is_empty() {
            (index + 1).to_string()
        } else {
            escape(&page.code)
        }
    };
    let _ = write!(
        out,
        "<h2>{}</h2><table><caption>{}</caption><tr><th scope=\"col\">{}</th>\
<th scope=\"col\">{}</th><th scope=\"col\">{}</th>",
        label("matrix.title"),
        label("matrix.caption"),
        label("url"),
        label("matrix.code"),
        label("status"),
    );
    for (index, page) in set.pages.iter().enumerate() {
        let _ = write!(
            out,
            "<th scope=\"col\"><abbr title=\"{}\">{}</abbr></th>",
            escape(&page.url),
            name(index)
        );
    }
    out.push_str("</tr>");
    for (row, page) in set.pages.iter().enumerate() {
        let _ = write!(
            out,
            "<tr><th scope=\"row\"><code>{}</code></th><td>{}</td><td>{}</td>",
            escape(&page.url),
            escape(&page.code),
            escape(&page.answer),
        );
        for value in set.matrix.get(row).into_iter().flatten() {
            let (class, text) = cell(*value);
            let _ = write!(out, "<td class=\"{class}\">{text}</td>");
        }
        out.push_str("</tr>");
    }
    let _ = write!(out, "</table><h3>{}</h3>", label("matrix.missing_pairs"));
    if set.missing.is_empty() {
        let _ = write!(out, "<p>{}</p>", label("matrix.none_missing"));
        return;
    }
    out.push_str("<ul class=\"missing-pairs\">");
    for pair in &set.missing {
        let _ = write!(
            out,
            "<li><code>{}</code> {} <code>{}</code></li>",
            escape(&pair.from),
            label("matrix.lacks"),
            escape(&pair.to),
        );
    }
    out.push_str("</ul>");
}

/// The views compared, the reference first in its row's header.
pub fn parity(out: &mut String, parity: &ParityReport) {
    let _ = write!(
        out,
        "<h2>{}</h2><table><caption>{}</caption><tr>",
        label("parity.title"),
        label("parity.caption"),
    );
    for column in [
        "parity.breakpoint",
        "parity.width",
        "page_title",
        "parity.h1",
        "parity.canonical",
        "parity.hreflang",
        "parity.structured_data",
        "parity.text_similarity",
    ] {
        let _ = write!(out, "<th scope=\"col\">{}</th>", label(column));
    }
    out.push_str("</tr>");
    for view in &parity.views {
        let reference = if view.view == parity.reference {
            format!(" ({})", label("parity.reference"))
        } else {
            String::new()
        };
        let similarity = view
            .text_similarity
            .map_or_else(|| "-".to_owned(), |percent| format!("{percent} %"));
        let _ = write!(
            out,
            "<tr><th scope=\"row\">{}{reference}</th><td>{}</td><td>{}</td><td>{}</td>\
<td><code>{}</code></td><td><code>{}</code></td><td>{}</td><td>{similarity}</td></tr>",
            escape(&view.breakpoint),
            view.width,
            escape(&view.title),
            escape(&view.h1),
            escape(&view.canonical),
            escape(&view.hreflang),
            escape(&view.structured_data),
        );
    }
    out.push_str("</table>");
}

#[cfg(test)]
#[path = "sets_tests.rs"]
mod tests;
