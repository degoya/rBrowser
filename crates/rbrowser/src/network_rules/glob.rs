//! URL globs as CDP's `Fetch.RequestPattern.urlPattern` reads them: `*` any characters (zero
//! or more, `/` included), `?` exactly one, `\` makes the next character literal. The whole URL
//! must match. The host matches again after CDP paused a request, so a rule's method and
//! resource type and the order of the rules count exactly as the plugin sent them.

#[derive(Clone, Copy, PartialEq, Eq)]
enum Token {
    Literal(char),
    One,
    Any,
}

fn tokens(pattern: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        tokens.push(match c {
            '\\' => Token::Literal(chars.next().unwrap_or('\\')),
            '*' => Token::Any,
            '?' => Token::One,
            other => Token::Literal(other),
        });
    }
    tokens
}

/// Whether `text` matches `pattern` as a whole.
#[must_use]
pub fn matches(pattern: &str, text: &str) -> bool {
    let tokens = tokens(pattern);
    let text: Vec<char> = text.chars().collect();
    // The classic single-backtrack wildcard match: at most O(pattern x text) steps.
    let (mut t, mut p) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while t < text.len() {
        match tokens.get(p) {
            Some(Token::Any) => {
                star = Some((p, t));
                p += 1;
            }
            Some(Token::One) => {
                p += 1;
                t += 1;
            }
            Some(Token::Literal(c)) if *c == text[t] => {
                p += 1;
                t += 1;
            }
            _ => match star {
                Some((star_p, star_t)) => {
                    p = star_p + 1;
                    t = star_t + 1;
                    star = Some((star_p, star_t + 1));
                }
                None => return false,
            },
        }
    }
    tokens[p.min(tokens.len())..]
        .iter()
        .all(|token| *token == Token::Any)
}

#[cfg(test)]
#[path = "glob_tests.rs"]
mod tests;
