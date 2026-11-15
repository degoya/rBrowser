//! The message format of the catalogue, shared with the Vue shell (`docs/i18n.md`): text with
//! named placeholders `{name}` and ICU plurals `{name, plural, =0 {...} one {...} other {...}}`,
//! where `#` inside a branch is the argument's value. There is no quoting: a literal `{` or `}`
//! cannot be written, and an apostrophe is just an apostrophe. A message that does not parse is
//! shown as written (the lint and the catalogue loader refuse it long before).

use crate::pseudo;

/// The CLDR plural categories a branch may name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluralCategory {
    Zero,
    One,
    Two,
    Few,
    Many,
    Other,
}

impl PluralCategory {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "zero" => Self::Zero,
            "one" => Self::One,
            "two" => Self::Two,
            "few" => Self::Few,
            "many" => Self::Many,
            "other" => Self::Other,
            _ => return None,
        })
    }
}

/// How the literal text of a message is written: as it is, or pseudo-localized (`en-XA`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Style {
    Plain,
    Pseudo,
}

#[derive(Debug)]
enum Part<'a> {
    Text(&'a str),
    /// `#` inside a plural branch.
    Hash,
    Arg {
        name: &'a str,
        source: &'a str,
    },
    Plural {
        name: &'a str,
        source: &'a str,
        branches: Vec<(Selector, Vec<Part<'a>>)>,
    },
}

#[derive(Debug, PartialEq)]
enum Selector {
    Exact(u64),
    Category(PluralCategory),
}

/// The plural category of `value` in English (phase 1 has no other language): `one` for the
/// integer 1 (also `-1`, `01`), `other` for everything else, `1.0` included - CLDR counts the
/// visible fraction digits. The shell applies the same rule to `String(value)`.
pub fn plural_category(value: &str) -> PluralCategory {
    let digits = value.trim();
    let digits = digits.strip_prefix('-').unwrap_or(digits);
    let integer = !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit());
    if integer && digits.parse::<u64>() == Ok(1) {
        PluralCategory::One
    } else {
        PluralCategory::Other
    }
}

/// Whether `message` is well-formed; the reason when it is not.
pub fn validate(message: &str) -> Result<(), String> {
    parse(message).map(|_| ())
}

/// `message` with its placeholders and plurals filled from `args`, in `style`. A missing
/// argument keeps its placeholder as written; a message that does not parse is returned as it is.
pub(crate) fn format(message: &str, args: &[(&str, &str)], style: Style) -> String {
    let Ok(parts) = parse(message) else {
        return message.to_owned();
    };
    let mut out = String::with_capacity(message.len());
    render(&parts, args, style, None, &mut out);
    match style {
        Style::Plain => out,
        Style::Pseudo => pseudo::pad(&out),
    }
}

fn argument<'a>(args: &[(&str, &'a str)], name: &str) -> Option<&'a str> {
    args.iter()
        .find(|(arg, _)| *arg == name)
        .map(|(_, value)| *value)
}

fn render(
    parts: &[Part<'_>],
    args: &[(&str, &str)],
    style: Style,
    hash: Option<&str>,
    out: &mut String,
) {
    for part in parts {
        match part {
            Part::Text(text) => match style {
                Style::Plain => out.push_str(text),
                Style::Pseudo => pseudo::accent(text, out),
            },
            Part::Hash => out.push_str(hash.unwrap_or("#")),
            Part::Arg { name, source } => out.push_str(argument(args, name).unwrap_or(source)),
            Part::Plural {
                name,
                source,
                branches,
            } => match argument(args, name) {
                Some(value) => {
                    if let Some(branch) = select(branches, value) {
                        render(branch, args, style, Some(value), out);
                    }
                }
                None => out.push_str(source),
            },
        }
    }
}

/// The branch for `value`: an exact `=n` first, then its category, then `other`.
fn select<'p, 'a>(
    branches: &'p [(Selector, Vec<Part<'a>>)],
    value: &str,
) -> Option<&'p [Part<'a>]> {
    let number = value.trim().parse::<f64>().ok();
    let exact = branches.iter().find(|(selector, _)| match selector {
        Selector::Exact(n) => number == Some(*n as f64),
        Selector::Category(_) => false,
    });
    let category = plural_category(value);
    let by = |wanted: PluralCategory| {
        branches
            .iter()
            .find(|(selector, _)| *selector == Selector::Category(wanted))
    };
    exact
        .or_else(|| by(category))
        .or_else(|| by(PluralCategory::Other))
        .map(|(_, parts)| parts.as_slice())
}

fn parse(message: &str) -> Result<Vec<Part<'_>>, String> {
    let mut parser = Parser {
        src: message,
        pos: 0,
    };
    parser.message(false)
}

struct Parser<'a> {
    src: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.src.as_bytes().get(self.pos).copied()
    }

    fn skip_space(&mut self) {
        while self.peek().is_some_and(|b| b.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, wanted: u8) -> Result<(), String> {
        if self.peek() == Some(wanted) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "expected `{}` at byte {}",
                wanted as char, self.pos
            ))
        }
    }

    /// `[A-Za-z_][A-Za-z0-9_]*`, or an error naming `what`.
    fn identifier(&mut self, what: &str) -> Result<&'a str, String> {
        let start = self.pos;
        if self
            .peek()
            .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        {
            while self
                .peek()
                .is_some_and(|b| b.is_ascii_alphanumeric() || b == b'_')
            {
                self.pos += 1;
            }
            Ok(&self.src[start..self.pos])
        } else {
            Err(format!("expected {what} at byte {start}"))
        }
    }

    /// A message up to its end, or - inside a plural branch - up to the `}` that closes it.
    fn message(&mut self, in_branch: bool) -> Result<Vec<Part<'a>>, String> {
        let mut parts = Vec::new();
        loop {
            match self.peek() {
                None if in_branch => return Err("a plural branch is not closed".to_owned()),
                None => return Ok(parts),
                Some(b'}') if in_branch => return Ok(parts),
                Some(b'}') => return Err(format!("unmatched `}}` at byte {}", self.pos)),
                Some(b'{') => parts.push(self.argument()?),
                Some(b'#') if in_branch => {
                    self.pos += 1;
                    parts.push(Part::Hash);
                }
                Some(_) => {
                    let start = self.pos;
                    while self
                        .peek()
                        .is_some_and(|b| b != b'{' && b != b'}' && !(in_branch && b == b'#'))
                    {
                        self.pos += 1;
                    }
                    parts.push(Part::Text(&self.src[start..self.pos]));
                }
            }
        }
    }

    /// `{name}` or `{name, plural, <branches>}`, the cursor on its `{`.
    fn argument(&mut self) -> Result<Part<'a>, String> {
        let start = self.pos;
        self.pos += 1;
        self.skip_space();
        let name = self.identifier("an argument name")?;
        self.skip_space();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            let source = &self.src[start..self.pos];
            return Ok(Part::Arg { name, source });
        }
        self.expect(b',')?;
        self.skip_space();
        let kind = self.identifier("`plural`")?;
        if kind != "plural" {
            return Err(format!("`{kind}` is not supported, only `plural`"));
        }
        self.skip_space();
        self.expect(b',')?;
        let branches = self.branches()?;
        let source = &self.src[start..self.pos];
        Ok(Part::Plural {
            name,
            source,
            branches,
        })
    }

    /// The branches of a plural up to and including its closing `}`.
    fn branches(&mut self) -> Result<Vec<(Selector, Vec<Part<'a>>)>, String> {
        let mut branches: Vec<(Selector, Vec<Part<'a>>)> = Vec::new();
        loop {
            self.skip_space();
            match self.peek() {
                None => return Err("a plural is not closed".to_owned()),
                Some(b'}') => {
                    self.pos += 1;
                    break;
                }
                _ => {}
            }
            let selector = self.selector()?;
            if branches.iter().any(|(seen, _)| *seen == selector) {
                return Err(format!("a plural names {selector:?} twice"));
            }
            self.skip_space();
            self.expect(b'{')?;
            let parts = self.message(true)?;
            self.expect(b'}')?;
            branches.push((selector, parts));
        }
        if branches
            .iter()
            .any(|(selector, _)| *selector == Selector::Category(PluralCategory::Other))
        {
            Ok(branches)
        } else {
            Err("a plural needs an `other` branch".to_owned())
        }
    }

    fn selector(&mut self) -> Result<Selector, String> {
        if self.peek() == Some(b'=') {
            self.pos += 1;
            let start = self.pos;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.pos += 1;
            }
            return self.src[start..self.pos]
                .parse()
                .map(Selector::Exact)
                .map_err(|_| format!("expected a number after `=` at byte {start}"));
        }
        let name = self.identifier("a plural category")?;
        PluralCategory::from_name(name)
            .map(Selector::Category)
            .ok_or_else(|| format!("`{name}` is no plural category"))
    }
}

#[cfg(test)]
#[path = "message_tests.rs"]
mod tests;
