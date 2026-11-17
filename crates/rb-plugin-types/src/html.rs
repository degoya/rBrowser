//! The one HTML escape of the app and its plugins (RB-080-28): every report, gallery and
//! placeholder page puts text into HTML through it.

/// `text` safe in HTML text and in a quoted attribute: `&`, `<`, `>`, `"` and `'` escaped
/// (`'` as `&#39;`, so a single-quoted attribute cannot be left either).
#[must_use]
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
#[path = "html_tests.rs"]
mod tests;
