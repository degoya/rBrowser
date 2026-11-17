//! The literal part of a JavaScript or TypeScript config file (RB-030-07): enough of the language
//! to read object, array, string and number literals, never to run anything. Every other
//! expression - a name, a call, a function, an operator, a template with `${}` - is kept as
//! [`Js::Dynamic`] with a short text for the hint. The reader is bounded: it always advances,
//! stops after [`MAX_TOKENS`] tokens and nests at most [`MAX_DEPTH`] levels.

/// The most tokens read from one file.
pub const MAX_TOKENS: usize = 200_000;
/// The deepest nesting read as literals.
pub const MAX_DEPTH: usize = 32;

/// A value of the config.
#[derive(Clone, Debug, PartialEq)]
pub enum Js {
    /// `{ key: value }`, in source order; a spread entry has the key `...`.
    Object(Vec<(String, Js)>),
    Array(Vec<Js>),
    Str(String),
    Num(f64),
    /// `...name.path` inside an object: the path.
    Spread(String),
    /// Anything else: a short text naming it (`require()`, `defaultTheme.screens`).
    Dynamic(String),
}

impl Js {
    /// The value of `key` in an object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Js> {
        match self {
            Self::Object(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Ident(String),
    Str(String),
    /// A template literal with `${}`.
    Interpolated,
    Num(f64),
    Spread,
    Punct(char),
}

impl Token {
    fn text(&self) -> String {
        match self {
            Self::Ident(name) => name.clone(),
            Self::Str(text) => format!("'{text}'"),
            Self::Interpolated => "`...`".to_owned(),
            Self::Num(number) => number.to_string(),
            Self::Spread => "...".to_owned(),
            Self::Punct(c) => c.to_string(),
        }
    }
}

/// The tokens of `text`; comments and white space left out.
fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < chars.len() && tokens.len() < MAX_TOKENS {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        if c.is_whitespace() {
            at += 1;
        } else if c == '/' && next == Some('/') {
            while at < chars.len() && chars[at] != '\n' {
                at += 1;
            }
        } else if c == '/' && next == Some('*') {
            at += 2;
            while at + 1 < chars.len() && !(chars[at] == '*' && chars[at + 1] == '/') {
                at += 1;
            }
            at = (at + 2).min(chars.len());
        } else if c == '"' || c == '\'' || c == '`' {
            let (token, end) = string(&chars, at);
            tokens.push(token);
            at = end;
        } else if c.is_ascii_digit() || (c == '.' && next.is_some_and(|n| n.is_ascii_digit())) {
            let start = at;
            while at < chars.len() && (chars[at].is_ascii_alphanumeric() || chars[at] == '.') {
                at += 1;
            }
            let literal: String = chars[start..at].iter().collect();
            let number = literal.replace('_', "").parse().unwrap_or(f64::NAN);
            tokens.push(Token::Num(number));
        } else if c == '.' && next == Some('.') && chars.get(at + 2) == Some(&'.') {
            tokens.push(Token::Spread);
            at += 3;
        } else if c.is_alphabetic() || c == '_' || c == '$' {
            let start = at;
            while at < chars.len()
                && (chars[at].is_alphanumeric() || chars[at] == '_' || chars[at] == '$')
            {
                at += 1;
            }
            tokens.push(Token::Ident(chars[start..at].iter().collect()));
        } else {
            tokens.push(Token::Punct(c));
            at += 1;
        }
    }
    tokens
}

/// The string or template starting at `start` and where the text goes on.
fn string(chars: &[char], start: usize) -> (Token, usize) {
    let quote = chars[start];
    let mut text = String::new();
    let mut interpolated = false;
    let mut at = start + 1;
    while at < chars.len() && chars[at] != quote {
        let c = chars[at];
        if c == '\n' && quote != '`' {
            break;
        }
        if c == '\\' {
            if let Some(escaped) = chars.get(at + 1) {
                text.push(*escaped);
            }
            at += 2;
            continue;
        }
        if quote == '`' && c == '$' && chars.get(at + 1) == Some(&'{') {
            interpolated = true;
        }
        text.push(c);
        at += 1;
    }
    let token = if interpolated {
        Token::Interpolated
    } else {
        Token::Str(text)
    };
    (token, (at + 1).min(chars.len()))
}

/// A cursor over the tokens.
struct Reader<'a> {
    tokens: &'a [Token],
    at: usize,
}

impl Reader<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.at)
    }

    fn is_punct(&self, c: char) -> bool {
        self.peek() == Some(&Token::Punct(c))
    }

    /// Whether the value that was just read ends here.
    fn at_value_end(&self) -> bool {
        matches!(
            self.peek(),
            None | Some(Token::Punct(',' | '}' | ']' | ')' | ';'))
        )
    }

    /// Skips one expression: up to a `,` or `;` or a closing bracket at its own level.
    fn skip_expression(&mut self) {
        let mut depth = 0_usize;
        while let Some(token) = self.peek() {
            match token {
                Token::Punct('{' | '[' | '(') => depth += 1,
                Token::Punct('}' | ']' | ')') if depth == 0 => return,
                Token::Punct('}' | ']' | ')') => depth -= 1,
                Token::Punct(',' | ';') if depth == 0 => return,
                _ => {}
            }
            self.at += 1;
        }
    }

    /// One value; anything but a literal as [`Js::Dynamic`].
    fn value(&mut self, depth: usize) -> Js {
        let start = self.at;
        let literal = if depth >= MAX_DEPTH {
            None
        } else {
            self.literal(depth)
        };
        // TypeScript's `{ ... } satisfies Config` and `as const` keep the literal.
        if literal.is_some()
            && matches!(self.peek(), Some(Token::Ident(word)) if word == "satisfies" || word == "as")
        {
            self.skip_expression();
        }
        match literal {
            Some(value) if self.at_value_end() => value,
            _ => {
                self.at = start;
                let name = self.describe();
                self.skip_expression();
                if self.at == start && self.peek().is_some() && !self.at_value_end() {
                    self.at += 1;
                }
                Js::Dynamic(name)
            }
        }
    }

    /// A short text for the expression at the cursor: a name path, with `()` for a call.
    fn describe(&self) -> String {
        let mut text = String::new();
        let mut at = self.at;
        while let Some(token) = self.tokens.get(at) {
            match token {
                Token::Ident(name) if text.is_empty() || text.ends_with('.') => text.push_str(name),
                Token::Punct('.') if !text.is_empty() && !text.ends_with('.') => text.push('.'),
                Token::Punct('(') if !text.is_empty() => {
                    text.push_str("()");
                    break;
                }
                Token::Punct('(') => return "(...) =>".to_owned(),
                _ if text.is_empty() => return token.text(),
                _ => break,
            }
            at += 1;
        }
        text
    }

    fn literal(&mut self, depth: usize) -> Option<Js> {
        match self.peek()? {
            Token::Punct('{') => Some(self.object(depth)),
            Token::Punct('[') => Some(self.array(depth)),
            Token::Str(text) => {
                let value = Js::Str(text.clone());
                self.at += 1;
                Some(value)
            }
            Token::Num(number) => {
                let value = Js::Num(*number);
                self.at += 1;
                Some(value)
            }
            _ => None,
        }
    }

    fn object(&mut self, depth: usize) -> Js {
        self.at += 1;
        let mut entries = Vec::new();
        loop {
            let before = self.at;
            match self.peek() {
                None => break,
                Some(Token::Punct('}')) => {
                    self.at += 1;
                    break;
                }
                Some(Token::Punct(',')) => self.at += 1,
                Some(Token::Spread) => {
                    self.at += 1;
                    match self.value(depth + 1) {
                        Js::Object(inner) => entries.extend(inner),
                        Js::Dynamic(path) => entries.push(("...".to_owned(), Js::Spread(path))),
                        other => entries.push(("...".to_owned(), other)),
                    }
                }
                Some(token) => {
                    let key = match token {
                        Token::Ident(name) | Token::Str(name) => Some(name.clone()),
                        Token::Num(number) => Some(number.to_string()),
                        _ => None,
                    };
                    self.entry(key, depth, &mut entries);
                }
            }
            if self.at == before {
                self.at += 1;
            }
        }
        Js::Object(entries)
    }

    /// One `key: value` (or shorthand, method, computed key) of an object.
    fn entry(&mut self, key: Option<String>, depth: usize, entries: &mut Vec<(String, Js)>) {
        let Some(key) = key else {
            // A computed key (`[name]: value`) or something unreadable.
            let name = self.describe();
            self.skip_expression();
            entries.push((format!("[{name}]"), Js::Dynamic(name)));
            return;
        };
        self.at += 1;
        if self.is_punct(':') {
            self.at += 1;
            let value = self.value(depth + 1);
            entries.push((key, value));
        } else {
            // `{ screens }` or `screens() { ... }`.
            self.skip_expression();
            entries.push((key.clone(), Js::Dynamic(key)));
        }
    }

    fn array(&mut self, depth: usize) -> Js {
        self.at += 1;
        let mut items = Vec::new();
        loop {
            let before = self.at;
            match self.peek() {
                None => break,
                Some(Token::Punct(']')) => {
                    self.at += 1;
                    break;
                }
                Some(Token::Punct(',')) => self.at += 1,
                Some(_) => items.push(self.value(depth + 1)),
            }
            if self.at == before {
                self.at += 1;
            }
        }
        Js::Array(items)
    }
}

/// A config file read for its literals.
pub struct Config {
    tokens: Vec<Token>,
}

impl Config {
    /// Reads `text`.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        Self {
            tokens: tokenize(text),
        }
    }

    /// The value of the first property `name: ...` of any object literal in the file.
    #[must_use]
    pub fn property(&self, name: &str) -> Option<Js> {
        let index = (1..self.tokens.len().saturating_sub(1)).find(|&index| {
            matches!(&self.tokens[index], Token::Ident(key) | Token::Str(key) if key == name)
                && self.tokens[index + 1] == Token::Punct(':')
                && matches!(self.tokens[index - 1], Token::Punct('{' | ','))
        })?;
        let mut reader = Reader {
            tokens: &self.tokens,
            at: index + 2,
        };
        Some(reader.value(1))
    }

    /// Whether the file exports an object literal: `module.exports = {` or `export default {`
    /// (also through `defineConfig({`).
    #[must_use]
    pub fn exports_object(&self) -> bool {
        let ident = |at: usize, word: &str| matches!(self.tokens.get(at), Some(Token::Ident(name)) if name == word);
        let punct = |at: usize, c: char| self.tokens.get(at) == Some(&Token::Punct(c));
        (0..self.tokens.len()).any(|at| {
            let after = if ident(at, "module")
                && punct(at + 1, '.')
                && ident(at + 2, "exports")
                && punct(at + 3, '=')
            {
                at + 4
            } else if ident(at, "export") && ident(at + 1, "default") {
                at + 2
            } else {
                return false;
            };
            punct(after, '{')
                || (matches!(self.tokens.get(after), Some(Token::Ident(_)))
                    && punct(after + 1, '(')
                    && punct(after + 2, '{'))
        })
    }
}

#[cfg(test)]
#[path = "js_tests.rs"]
mod tests;
