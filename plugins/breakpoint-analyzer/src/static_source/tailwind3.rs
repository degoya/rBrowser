//! Tailwind v3: `theme.screens` and `theme.extend.screens` of `tailwind.config.{js,cjs,mjs,ts}`,
//! read as literals ([`super::js`]) - the file is never run.
//!
//! - No `theme.screens`: Tailwind's default screens; `theme.screens` replaces them;
//!   `theme.extend.screens` adds to them (a known name takes the new value).
//! - A screen is a length string (`'768px'`) or `{ min: '768px' }`; `...defaultTheme.screens`
//!   stands for the defaults.
//! - `{ min, max }`, `{ max }`, `{ raw }` and lists of them are no `min-width` boundary: a
//!   `range` hint. A name, a call, a template with `${}`: a `dynamic` hint. `presets`: a hint
//!   that their screens are not read.

use super::js::{Config, Js};
use super::{FileScan, Hint, literal_px};
use crate::suggest::Framework;

/// Tailwind v3's default screens.
pub const DEFAULTS: [(&str, f64); 5] = [
    ("sm", 640.0),
    ("md", 768.0),
    ("lg", 1024.0),
    ("xl", 1280.0),
    ("2xl", 1536.0),
];

/// The screens of the config at `path`.
#[must_use]
pub fn scan(path: &str, text: &str, root_font_size: f64) -> FileScan {
    let config = Config::parse(text);
    let mut reader = Screens {
        path,
        root_font_size,
        found: FileScan::default(),
    };
    if config.property("presets").is_some() {
        reader.hint("presets", "presets");
    }
    let screens = match config.property("theme") {
        None if config.exports_object() => Some(defaults()),
        None => {
            reader.hint("dynamic", "module.exports");
            None
        }
        Some(theme @ Js::Object(_)) => reader.theme(&theme),
        Some(other) => {
            reader.hint("dynamic", &name_of(&other, "theme"));
            None
        }
    };
    for (label, px) in screens.unwrap_or_default() {
        reader
            .found
            .declare(path, &label, px, Some(Framework::Tailwind), false);
    }
    reader.found
}

fn defaults() -> Vec<(String, f64)> {
    DEFAULTS
        .iter()
        .map(|(label, px)| ((*label).to_owned(), *px))
        .collect()
}

/// The text a hint names a value by.
fn name_of(value: &Js, key: &str) -> String {
    match value {
        Js::Dynamic(name) if !name.is_empty() => format!("{key}: {name}"),
        _ => key.to_owned(),
    }
}

struct Screens<'a> {
    path: &'a str,
    root_font_size: f64,
    found: FileScan,
}

impl Screens<'_> {
    fn hint(&mut self, kind: &str, name: &str) {
        self.found.hints.push(Hint::new(self.path, kind, name));
    }

    /// The screens of a `theme` object; `None` when they cannot be read.
    fn theme(&mut self, theme: &Js) -> Option<Vec<(String, f64)>> {
        let mut screens = match theme.get("screens") {
            None => defaults(),
            Some(Js::Dynamic(name)) if name.ends_with(".screens") => defaults(),
            Some(value @ Js::Object(_)) => self.object(value),
            Some(other) => {
                self.hint("dynamic", &name_of(other, "screens"));
                return None;
            }
        };
        match theme
            .get("extend")
            .map(|extend| (extend, extend.get("screens")))
        {
            None | Some((Js::Object(_), None)) => {}
            Some((_, Some(extended @ Js::Object(_)))) => {
                for (label, px) in self.object(extended) {
                    match screens.iter_mut().find(|(known, _)| *known == label) {
                        Some(known) => known.1 = px,
                        None => screens.push((label, px)),
                    }
                }
            }
            Some((_, Some(other))) => self.hint("dynamic", &name_of(other, "extend.screens")),
            Some((other, None)) => self.hint("dynamic", &name_of(other, "extend")),
        }
        Some(screens)
    }

    /// The readable screens of a `screens` object, in source order.
    fn object(&mut self, screens: &Js) -> Vec<(String, f64)> {
        let Js::Object(entries) = screens else {
            return Vec::new();
        };
        let mut found: Vec<(String, f64)> = Vec::new();
        for (label, value) in entries {
            let px = match value {
                Js::Spread(path) if path.ends_with("screens") => {
                    found.extend(defaults());
                    continue;
                }
                Js::Str(text) => literal_px(text, self.root_font_size),
                Js::Object(_) if value.get("raw").is_some() || value.get("max").is_some() => {
                    self.hint("range", label);
                    continue;
                }
                Js::Object(_) => match value.get("min") {
                    Some(Js::Str(text)) => literal_px(text, self.root_font_size),
                    _ => None,
                },
                Js::Array(_) => {
                    self.hint("range", label);
                    continue;
                }
                _ => None,
            };
            match px {
                Some(px) => match found.iter_mut().find(|(known, _)| known == label) {
                    Some(known) => known.1 = px,
                    None => found.push((label.clone(), px)),
                },
                None => {
                    let name = match value {
                        Js::Spread(path) => format!("...{path}"),
                        _ => label.clone(),
                    };
                    self.hint("dynamic", &name);
                }
            }
        }
        found
    }
}

#[cfg(test)]
#[path = "tailwind3_tests.rs"]
mod tests;
