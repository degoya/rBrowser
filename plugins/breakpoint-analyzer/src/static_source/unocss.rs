//! UnoCSS (RB-090-50): `theme.breakpoints` (`presetUno`, `presetWind3`, `presetMini`) and
//! `theme.breakpoint` (`presetWind4`) of `uno.config.{js,cjs,mjs,ts}` or
//! `unocss.config.{js,cjs,mjs,ts}`, read as literals ([`super::js`]) - the file is never run.
//!
//! UnoCSS replaces its preset's breakpoints with the config's object instead of merging the two,
//! so the object is all there is. Without one the preset's defaults hold - Tailwind's widths,
//! which the media queries already name when they are recognised - and nothing is declared.
//! A value that is no length literal is a `dynamic` hint; names such as `2xl` and `3xl` stay as
//! they are written.

use super::js::{Config, Js};
use super::{FileScan, Hint, literal_px};

/// The keys of the theme that hold the breakpoints, `presetWind4`'s first.
const KEYS: [&str; 2] = ["breakpoint", "breakpoints"];

/// The breakpoints of the UnoCSS config at `path`.
#[must_use]
pub fn scan(path: &str, text: &str, root_font_size: f64) -> FileScan {
    let mut found = FileScan::default();
    let Some(theme @ Js::Object(_)) = Config::parse(text).property("theme") else {
        return found;
    };
    let Some((key, breakpoints)) = KEYS
        .iter()
        .find_map(|key| theme.get(key).map(|value| (*key, value)))
    else {
        return found;
    };
    let Js::Object(entries) = breakpoints else {
        found.hints.push(Hint::new(path, "dynamic", key));
        return found;
    };
    for (label, value) in entries {
        let px = match value {
            Js::Str(text) => literal_px(text, root_font_size),
            _ => None,
        };
        match (px, value) {
            (Some(px), _) => found.declare(path, label, px, None, false),
            (None, Js::Spread(spread)) => {
                found
                    .hints
                    .push(Hint::new(path, "dynamic", &format!("...{spread}")));
            }
            (None, _) => found.hints.push(Hint::new(path, "dynamic", label)),
        }
    }
    found
}

#[cfg(test)]
#[path = "unocss_tests.rs"]
mod tests;
