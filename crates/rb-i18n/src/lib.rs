//! Translation keys for the Rust core: [`t`], [`t_with`] and the [`t!`] macro over the catalogue
//! shared with the Vue shell (contract 0.1, `docs/contracts-0.1.md`; the format is
//! `docs/i18n.md`).
//!
//! **Keys** are `<area>.<thing>`: at least two dot-separated segments; the first is the area in
//! lower case (`toolbar`, `settings`, `config`, `commands`, `grid`, `emulation`, `shortcuts`,
//! `plugin`), the following segments start with a lower-case letter or digit and may hold
//! letters, digits, `_` and `-` (setting keys keep their camelCase:
//! `settings.key.sync.leaderOnlySubmit.label`). Plugin keys are `plugin.<plugin-id>.*`.
//!
//! **Catalogue**: `locales/en.json`, one file for core and shell, nested by segment
//! (`{"toolbar": {"reload": "Reload"}}` holds `toolbar.reload`), compiled into the binary.
//! Values are messages: named placeholders `{name}` and ICU plurals
//! `{count, plural, one {# view} other {# views}}` ([`plural_category`] picks the branch).
//! **A missing key shows itself** (`t("toolbar.nope")` is `"toolbar.nope"`), so a missing text is
//! visible; `scripts/check-i18n.sh` fails on it before it ships.
//!
//! **Plugin locales**: the host hands a plugin's `locales/<lang>.json` to
//! [`register_plugin_locale`]; its keys must lie below `plugin.<id>.`, so a plugin can never
//! override a core key or another plugin's. [`t`] finds them like core keys, and
//! [`plugin_texts`] gives the host a plugin's texts for its panels and settings.
//!
//! **In a wave** no job edits `locales/en.json`: each job delivers its keys as
//! `locales/parts/<job-id>.json` (same nested shape, e.g. `locales/parts/RB-010-05.json`) and
//! the coordinator merges them with `python3 scripts/lib/merge_locales.py`; a key defined twice
//! with different texts is a merge conflict to resolve, not to overwrite.

use std::{
    collections::BTreeMap,
    sync::{OnceLock, PoisonError, RwLock},
};

mod catalogue;
mod message;
mod pseudo;

pub use catalogue::{Catalogue, I18nError, MAX_PLUGIN_LOCALE_BYTES, plugin_catalogue};
pub use message::{PluralCategory, plural_category, validate as validate_message};
pub use pseudo::pseudo_localize;

/// The English catalogue, relative to the repository root. Shared by Rust and the Vue shell.
pub const CATALOGUE_PATH: &str = "locales/en.json";
/// Where wave jobs deliver their keys, one `<job-id>.json` each.
pub const PARTS_DIR: &str = "locales/parts";
/// The only locale of phase 1.
pub const DEFAULT_LOCALE: &str = "en";
/// The pseudo-locale for layout tests: English, accented, bracketed and 40 % longer. It is no
/// choice of `ui.language`; tests and the developer switch it on.
pub const PSEUDO_LOCALE: &str = "en-XA";

/// `locales/en.json`, compiled in.
const CATALOGUE_JSON: &str = include_str!("../../../locales/en.json");

/// The path of the part file job `job_id` delivers (`RB-010-05` -> `locales/parts/RB-010-05.json`).
pub fn part_path(job_id: &str) -> String {
    format!("{PARTS_DIR}/{job_id}.json")
}

/// `t!("key")` is [`t`]`("key")`; `t!("key", count = n, name = x)` is [`t_with`] with each value
/// turned into text (`to_string`).
#[macro_export]
macro_rules! t {
    ($key:literal $(,)?) => {
        $crate::t($key)
    };
    ($key:literal, $($name:ident = $value:expr),+ $(,)?) => {
        $crate::t_with($key, &[$((stringify!($name), $value.to_string().as_str())),+])
    };
}

/// The text of `key` in the UI language; the key itself when it is missing (so a missing text
/// is visible, and the lint catches it).
pub fn t(key: &str) -> String {
    t_with(key, &[])
}

/// The text of `key` with each `{name}` and plural filled from `args`; the key itself when it
/// is missing. A placeholder without an argument stays as written.
pub fn t_with(key: &str, args: &[(&str, &str)]) -> String {
    match lookup(key) {
        Some(text) => format_message(&text, args),
        None => key.to_owned(),
    }
}

/// Formats the message `text` (from a catalogue) with `args` in the UI language - for a text a
/// caller already holds, such as a plugin's.
pub fn format_message(text: &str, args: &[(&str, &str)]) -> String {
    message::format(text, args, style())
}

/// The locales `ui.language` offers: one per catalogue shipped (phase 1: `en`).
pub fn available_locales() -> &'static [&'static str] {
    &[DEFAULT_LOCALE]
}

/// Switches the UI language: one of [`available_locales`] or [`PSEUDO_LOCALE`].
pub fn set_locale(locale: &str) -> Result<(), I18nError> {
    let pseudo = match locale {
        PSEUDO_LOCALE => true,
        _ if available_locales().contains(&locale) => false,
        _ => return Err(I18nError::UnknownLocale(locale.to_owned())),
    };
    write_state().pseudo = pseudo;
    Ok(())
}

/// The UI language.
pub fn locale() -> &'static str {
    if read_state().pseudo {
        PSEUDO_LOCALE
    } else {
        DEFAULT_LOCALE
    }
}

/// The core catalogue, `locales/en.json` as compiled in. One that does not parse is empty, every
/// key then shows itself; `the_compiled_catalogue_is_valid` keeps that from shipping.
pub fn core_catalogue() -> &'static Catalogue {
    static CORE: OnceLock<Catalogue> = OnceLock::new();
    CORE.get_or_init(|| Catalogue::from_json(CATALOGUE_JSON).unwrap_or_default())
}

/// Loads `locales/<lang>.json` of plugin `plugin_id` (see [`plugin_catalogue`]), replacing what
/// that plugin registered for `lang` before; the number of its keys. `lang` is a two-letter tag;
/// phase 1 shows the `en` texts.
pub fn register_plugin_locale(
    plugin_id: &str,
    lang: &str,
    json: &[u8],
) -> Result<usize, I18nError> {
    if lang.len() != 2 || !lang.bytes().all(|b| b.is_ascii_lowercase()) {
        return Err(I18nError::UnknownLocale(lang.to_owned()));
    }
    let catalogue = plugin_catalogue(plugin_id, json)?;
    let keys = catalogue.len();
    write_state()
        .plugins
        .entry(plugin_id.to_owned())
        .or_default()
        .insert(lang.to_owned(), catalogue);
    Ok(keys)
}

/// Forgets every locale of plugin `plugin_id` (it was disabled or removed).
pub fn unregister_plugin_locales(plugin_id: &str) {
    write_state().plugins.remove(plugin_id);
}

/// The text of `key` in plugin `plugin_id`'s English catalogue, unformatted; `None` when the
/// plugin or the key is unknown. One lookup, no copy of the catalogue (the context menu reads a
/// title per right click, RB-080-43).
pub fn plugin_text(plugin_id: &str, key: &str) -> Option<String> {
    read_state()
        .plugins
        .get(plugin_id)
        .and_then(|locales| locales.get(DEFAULT_LOCALE))
        .and_then(|catalogue| catalogue.get(key))
        .map(str::to_owned)
}

/// The texts of plugin `plugin_id` in the UI language, by full key, as messages (unformatted) -
/// what the host hands to the shell for the plugin's panels and settings.
pub fn plugin_texts(plugin_id: &str) -> BTreeMap<String, String> {
    read_state()
        .plugins
        .get(plugin_id)
        .and_then(|locales| locales.get(DEFAULT_LOCALE))
        .map(|catalogue| {
            catalogue
                .iter()
                .map(|(key, text)| (key.to_owned(), text.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

/// The language state and the plugins' catalogues by id and language.
struct State {
    pseudo: bool,
    plugins: BTreeMap<String, BTreeMap<String, Catalogue>>,
}

static STATE: RwLock<State> = RwLock::new(State {
    pseudo: false,
    plugins: BTreeMap::new(),
});

// A panic while the lock was held cannot leave the state half-written (every write is one
// assignment or insert), so a poisoned lock is used as it is.
fn read_state() -> std::sync::RwLockReadGuard<'static, State> {
    STATE.read().unwrap_or_else(PoisonError::into_inner)
}

fn write_state() -> std::sync::RwLockWriteGuard<'static, State> {
    STATE.write().unwrap_or_else(PoisonError::into_inner)
}

fn style() -> message::Style {
    if read_state().pseudo {
        message::Style::Pseudo
    } else {
        message::Style::Plain
    }
}

/// The message of `key`: a plugin's key from that plugin's catalogue, any other from the core.
fn lookup(key: &str) -> Option<String> {
    if let Some(plugin_id) = key
        .strip_prefix("plugin.")
        .and_then(|rest| rest.split('.').next())
    {
        let state = read_state();
        let found = state
            .plugins
            .get(plugin_id)
            .and_then(|locales| locales.get(DEFAULT_LOCALE))
            .and_then(|catalogue| catalogue.get(key));
        if let Some(text) = found {
            return Some(text.to_owned());
        }
    }
    core_catalogue().get(key).map(str::to_owned)
}

/// Segments no key may have: the shell nests keys into objects, and these would walk into an
/// object's prototype there (`__proto__` already fails the character rule).
const UNSAFE_SEGMENTS: [&str; 3] = ["__proto__", "constructor", "prototype"];

/// Whether `key` follows the naming convention above; no segment is `__proto__`, `constructor`
/// or `prototype`.
pub fn is_well_formed_key(key: &str) -> bool {
    if key
        .split('.')
        .any(|segment| UNSAFE_SEGMENTS.contains(&segment))
    {
        return false;
    }
    let mut segments = key.split('.');
    let area_ok = segments.next().is_some_and(|area| {
        area.starts_with(|c: char| c.is_ascii_lowercase())
            && area
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    });
    let mut rest = segments.peekable();
    let has_rest = rest.peek().is_some();
    area_ok
        && has_rest
        && rest.all(|segment| {
            segment.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
