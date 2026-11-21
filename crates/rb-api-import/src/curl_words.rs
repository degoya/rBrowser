//! A command line cut into words as the shells do: bash (`'...'`, `$'...'` with its escapes,
//! `"..."`, `\` before a line break), cmd (`^` before any character, `^` at a line's end) and
//! PowerShell (`` ` `` at a line's end).

use std::iter::Peekable;
use std::str::Chars;

/// `text` with cmd's carets taken out: `^` before a line break joins the lines, `^x` is `x`.
fn uncaret(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '^' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('\r') => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
            }
            Some('\n') | None => {}
            Some(next) => out.push(next),
        }
    }
    out
}

fn hex(chars: &mut Peekable<Chars<'_>>, len: usize) -> Option<char> {
    let digits: String = (0..len)
        .map_while(|_| chars.next_if(char::is_ascii_hexdigit))
        .collect();
    u32::from_str_radix(&digits, 16)
        .ok()
        .and_then(char::from_u32)
}

fn ansi_escape(chars: &mut Peekable<Chars<'_>>, out: &mut String) {
    let Some(c) = chars.next() else {
        return;
    };
    match c {
        'n' => out.push('\n'),
        't' => out.push('\t'),
        'r' => out.push('\r'),
        '0' => out.push('\0'),
        'x' => out.extend(hex(chars, 2)),
        'u' => out.extend(hex(chars, 4)),
        'U' => out.extend(hex(chars, 8)),
        other => out.push(other),
    }
}

/// The words of a command line, the shells' quoting undone.
///
/// # Errors
///
/// A quote left open.
pub(crate) fn words(text: &str) -> Result<Vec<String>, String> {
    let text = if text.contains("^\"") || text.contains("^\n") || text.contains("^\r\n") {
        uncaret(text)
    } else {
        text.to_owned()
    };
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            c if c.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '\\' | '`' => match chars.next() {
                Some('\n') => {}
                Some('\r') if chars.peek() == Some(&'\n') => {
                    chars.next();
                }
                Some(next) if c == '\\' => {
                    word.push(next);
                    started = true;
                }
                Some(next) => {
                    word.push(c);
                    word.push(next);
                    started = true;
                }
                None => {}
            },
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(inner) => word.push(inner),
                        None => return Err("a quote is left open".to_owned()),
                    }
                }
            }
            '$' if chars.peek() == Some(&'\'') => {
                chars.next();
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some('\\') => ansi_escape(&mut chars, &mut word),
                        Some(inner) => word.push(inner),
                        None => return Err("a quote is left open".to_owned()),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(next @ ('"' | '\\' | '$' | '`')) => word.push(next),
                            Some('\n') => {}
                            Some(next) => {
                                word.push('\\');
                                word.push(next);
                            }
                            None => return Err("a quote is left open".to_owned()),
                        },
                        Some(inner) => word.push(inner),
                        None => return Err("a quote is left open".to_owned()),
                    }
                }
            }
            other => {
                word.push(other);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    Ok(words)
}
