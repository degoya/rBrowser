//! JavaScript for `fetch` and Playwright: a word as a string literal - single quotes, or a
//! template literal reading `process.env.<name>` where the word holds an environment variable -
//! and an object literal over lines.

use crate::word::{Seg, Word};

fn escape(c: char, quote: char, out: &mut String) {
    match c {
        '\\' => out.push_str("\\\\"),
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        '\u{2028}' => out.push_str("\\u2028"),
        '\u{2029}' => out.push_str("\\u2029"),
        c if c == quote => {
            out.push('\\');
            out.push(c);
        }
        // In a template literal `$` would open `${`.
        '$' if quote == '`' => out.push_str("\\$"),
        c if u32::from(c) < 0x20 || c == '\u{7f}' => {
            out.push_str(&format!("\\u{:04x}", u32::from(c)));
        }
        c => out.push(c),
    }
}

/// `word` as a JavaScript string expression.
pub(crate) fn string(word: &Word) -> String {
    if let Some(text) = word.text_only() {
        let mut out = String::from("'");
        for c in text.chars() {
            escape(c, '\'', &mut out);
        }
        out.push('\'');
        return out;
    }
    let mut out = String::from("`");
    for seg in &word.segs {
        match seg {
            Seg::Text(text) => {
                for c in text.chars() {
                    escape(c, '`', &mut out);
                }
            }
            Seg::Env(name) => out.push_str(&format!("${{process.env.{name}}}")),
        }
    }
    out.push('`');
    out
}

/// A plain text as a string literal.
pub(crate) fn lit(text: &str) -> String {
    string(&Word::lit(text))
}

/// `entries` (key and value code) as an object literal whose closing brace is indented by
/// `indent` spaces, an entry a line; `{}` for none.
pub(crate) fn object(entries: &[(String, String)], indent: usize) -> String {
    if entries.is_empty() {
        return "{}".to_owned();
    }
    let inner = " ".repeat(indent + 2);
    let mut out = String::from("{\n");
    for (key, value) in entries {
        out.push_str(&format!("{inner}{key}: {value},\n"));
    }
    out.push_str(&" ".repeat(indent));
    out.push('}');
    out
}

/// Whether two of `names` are the same, case aside.
pub(crate) fn repeats<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    let mut seen: Vec<String> = Vec::new();
    for name in names {
        let lower = name.to_ascii_lowercase();
        if seen.contains(&lower) {
            return true;
        }
        seen.push(lower);
    }
    false
}

#[cfg(test)]
#[path = "js_tests.rs"]
mod tests;
