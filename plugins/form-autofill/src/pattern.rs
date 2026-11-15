//! A string that matches an `<input pattern>`, for the common subset: literals, escapes (`\d`,
//! `\w`, `\s`, an escaped character), `.`, classes with ranges and negation, groups (`(...)`,
//! `(?:...)`) with alternatives, and the quantifiers `?`, `*`, `+`, `{n}`, `{n,}`, `{n,m}` (lazy
//! ones too). Lookarounds, backreferences and anything else give no sample: the page script then
//! keeps the generated value and reports the field as not matching. The page script checks every
//! sample against the real pattern before it keeps it.

use crate::rng::Rng;

/// The longest sample made.
const MAX_LEN: usize = 1000;
/// The deepest nesting of groups followed.
const MAX_DEPTH: usize = 32;

struct Parser<'a> {
    chars: Vec<char>,
    at: usize,
    rng: &'a mut Rng,
    depth: usize,
}

const WORD: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'k', 'm', 'r', 's', 't', 'x', 'A', 'B', 'K', 'X', '0', '1', '2', '5',
    '7', '_',
];
const DIGIT: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
/// What a negated class or `.` tries, in order.
const FALLBACK: &[char] = &['a', 'x', 'A', 'X', '0', '1', '-', '_', ' '];

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.at).copied()
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    /// alternatives: sequence ('|' sequence)*; the first alternative is taken.
    fn alternatives(&mut self) -> Option<String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return None;
        }
        let first = self.sequence()?;
        while self.eat('|') {
            self.sequence()?;
        }
        self.depth -= 1;
        Some(first)
    }

    fn sequence(&mut self) -> Option<String> {
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            let (low, high) = self.quantifier()?;
            let times = if high == Some(0) { 0 } else { low.max(1) };
            // A group is read once; its text repeats.
            for _ in 0..times {
                out.push_str(&atom);
                if out.chars().count() > MAX_LEN {
                    return None;
                }
            }
        }
        Some(out)
    }

    fn quantifier(&mut self) -> Option<(usize, Option<usize>)> {
        let bounds = match self.peek() {
            Some('?') => {
                self.at += 1;
                (1, Some(1))
            }
            Some('*') | Some('+') => {
                self.at += 1;
                (1, None)
            }
            Some('{') => {
                self.at += 1;
                let low = self.integer()?;
                let high = if self.eat(',') {
                    if self.peek() == Some('}') {
                        None
                    } else {
                        Some(self.integer()?)
                    }
                } else {
                    Some(low)
                };
                if !self.eat('}') || high.is_some_and(|high| high < low) {
                    return None;
                }
                (low, high)
            }
            _ => return Some((1, Some(1))),
        };
        self.eat('?');
        Some(bounds)
    }

    fn integer(&mut self) -> Option<usize> {
        let start = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.at += 1;
        }
        let text: String = self.chars[start..self.at].iter().collect();
        text.parse().ok().filter(|n| *n <= MAX_LEN)
    }

    fn atom(&mut self) -> Option<String> {
        let c = self.peek()?;
        self.at += 1;
        match c {
            '(' => {
                if self.eat('?') && !self.eat(':') {
                    return None;
                }
                let inner = self.alternatives()?;
                self.eat(')').then_some(inner)
            }
            '[' => self.class().map(String::from),
            '\\' => self.escape().map(String::from),
            '.' => Some(String::from(FALLBACK[0])),
            '^' | '$' => Some(String::new()),
            '*' | '+' | '?' | '{' | '}' | ']' => None,
            other => Some(String::from(other)),
        }
    }

    fn escape(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.at += 1;
        Some(match c {
            'd' => *self.rng.pick(DIGIT),
            'w' => *self.rng.pick(WORD),
            's' => ' ',
            'D' | 'S' => 'a',
            'W' => '-',
            '1'..='9' | 'b' | 'B' | 'k' | 'p' | 'P' | 'u' | 'x' | 'c' => return None,
            'n' => '\n',
            't' => '\t',
            other => other,
        })
    }

    /// The characters of one class member: a range, an escape or a character.
    fn member(&mut self) -> Option<Vec<char>> {
        let c = self.peek()?;
        self.at += 1;
        let from = match c {
            '\\' => match self.peek()? {
                'd' => {
                    self.at += 1;
                    return Some(DIGIT.to_vec());
                }
                'w' => {
                    self.at += 1;
                    return Some(WORD.to_vec());
                }
                's' => {
                    self.at += 1;
                    return Some(vec![' ']);
                }
                _ => self.escape()?,
            },
            other => other,
        };
        if self.peek() == Some('-') && self.chars.get(self.at + 1).is_some_and(|c| *c != ']') {
            self.at += 1;
            let to = self.peek()?;
            self.at += 1;
            let to = if to == '\\' { self.escape()? } else { to };
            if to < from {
                return None;
            }
            return Some((from..=to).take(128).collect());
        }
        Some(vec![from])
    }

    fn class(&mut self) -> Option<char> {
        let negated = self.eat('^');
        let mut set = Vec::new();
        let mut first = true;
        loop {
            match self.peek()? {
                ']' if !first => {
                    self.at += 1;
                    break;
                }
                _ => set.extend(self.member()?),
            }
            first = false;
        }
        if negated {
            FALLBACK.iter().copied().find(|c| !set.contains(c))
        } else if set.is_empty() {
            None
        } else {
            Some(*self.rng.pick(&set))
        }
    }
}

/// A string matching `pattern` (as a whole, the way `pattern` is applied), or none.
#[must_use]
pub fn sample(pattern: &str, rng: &mut Rng) -> Option<String> {
    let mut parser = Parser {
        chars: pattern.chars().collect(),
        at: 0,
        rng,
        depth: 0,
    };
    let out = parser.alternatives()?;
    (parser.at == parser.chars.len()).then_some(out)
}

#[cfg(test)]
#[path = "pattern_tests.rs"]
mod tests;
