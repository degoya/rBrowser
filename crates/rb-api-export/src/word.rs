//! A word of the code before it is quoted: text and the environment variables in it. A
//! template's variables become [`Seg::Env`] under [`Variables::Environment`] and stay text
//! (`{{name}}`) under [`Variables::Template`]; [`Words`] keeps the names read, for the panel's
//! "set these first".

use crate::Variables;
use crate::template::{env_name, pieces};

/// A piece of a word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Seg {
    Text(String),
    /// An environment variable, by a name of letters, digits and `_`.
    Env(String),
}

/// A word: its pieces, adjacent texts joined.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Word {
    pub segs: Vec<Seg>,
}

impl Word {
    /// A word of plain text.
    pub fn lit(text: &str) -> Self {
        let mut word = Self::default();
        word.push_text(text);
        word
    }

    pub fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if let Some(Seg::Text(last)) = self.segs.last_mut() {
            last.push_str(text);
        } else {
            self.segs.push(Seg::Text(text.to_owned()));
        }
    }

    /// The text of a word without variables.
    pub fn text_only(&self) -> Option<String> {
        match self.segs.as_slice() {
            [] => Some(String::new()),
            [Seg::Text(text)] => Some(text.clone()),
            _ => None,
        }
    }

    /// Whether any text piece holds one of `chars`.
    pub fn has_any(&self, chars: &[char]) -> bool {
        self.segs
            .iter()
            .any(|seg| matches!(seg, Seg::Text(text) if text.contains(chars)))
    }
}

/// Makes the words of one piece of code.
pub(crate) struct Words {
    style: Variables,
    env: Vec<String>,
}

impl Words {
    pub fn new(style: Variables) -> Self {
        Self {
            style,
            env: Vec::new(),
        }
    }

    /// The word of `template`.
    pub fn of(&mut self, template: &str) -> Word {
        let mut word = Word::default();
        for piece in pieces(template) {
            let name = match self.style {
                Variables::Environment => env_name(&piece),
                Variables::Template => None,
            };
            match (name, piece) {
                (Some(name), _) => {
                    if !self.env.contains(&name) {
                        self.env.push(name.clone());
                    }
                    word.segs.push(Seg::Env(name));
                }
                (None, crate::template::Piece::Text(text)) => word.push_text(text),
                (
                    None,
                    crate::template::Piece::Var { raw, .. }
                    | crate::template::Piece::Dynamic { raw, .. },
                ) => word.push_text(raw),
            }
        }
        word
    }

    /// The environment variables read, in order.
    pub fn into_env(self) -> Vec<String> {
        self.env
    }
}

#[cfg(test)]
#[path = "word_tests.rs"]
mod tests;
