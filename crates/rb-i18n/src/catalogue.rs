//! A catalogue: the nested JSON of `locales/<lang>.json` read into dotted keys, every key
//! well-formed, every text a valid message (`message.rs`), no key twice - a JSON map would keep
//! the last of two equal keys without a word, so one of the two texts would never show.

use std::{collections::BTreeMap, fmt};

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, Visitor};

use crate::{is_well_formed_key, message};

/// The largest `locales/<lang>.json` a plugin may ship: generous for a few thousand short texts.
pub const MAX_PLUGIN_LOCALE_BYTES: usize = 256 * 1024;

/// Why a catalogue, a plugin locale or a locale choice was refused. These reach a developer (a
/// catalogue in the tree, a plugin author's file) or the plugin host, which shows its own key.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum I18nError {
    #[error("the catalogue is not an object of nested texts: {0}")]
    Json(String),
    #[error("the key `{0}` is given twice")]
    Duplicate(String),
    #[error("`{0}` is no well-formed key (`<area>.<thing>`)")]
    BadKey(String),
    #[error("the text of `{key}` is no valid message: {reason}")]
    BadMessage { key: String, reason: String },
    #[error("the key `{0}` already has a different text")]
    Conflict(String),
    #[error("plugin `{plugin}` may only define keys below `plugin.{plugin}.`, not `{key}`")]
    ForeignKey { plugin: String, key: String },
    #[error("a plugin locale may hold at most {limit} bytes")]
    TooLarge { limit: usize },
    #[error("`{0}` is no plugin id")]
    BadPluginId(String),
    #[error("there is no catalogue for the locale `{0}`")]
    UnknownLocale(String),
}

/// Texts by dotted key, in key order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Catalogue {
    texts: BTreeMap<String, String>,
}

impl Catalogue {
    /// Reads a nested catalogue (`{"toolbar": {"reload": "Reload"}}`) and checks every entry.
    pub fn from_json(json: &str) -> Result<Self, I18nError> {
        let mut entries = Vec::new();
        let mut deserializer = serde_json::Deserializer::from_str(json);
        Flatten {
            prefix: None,
            out: &mut entries,
        }
        .deserialize(&mut deserializer)
        .and_then(|()| deserializer.end())
        .map_err(|error| I18nError::Json(error.to_string()))?;
        let mut texts = BTreeMap::new();
        for (key, text) in entries {
            if !is_well_formed_key(&key) {
                return Err(I18nError::BadKey(key));
            }
            let checked = if text.trim().is_empty() {
                Err("the text is empty".to_owned())
            } else {
                message::validate(&text)
            };
            if let Err(reason) = checked {
                return Err(I18nError::BadMessage { key, reason });
            }
            if texts.contains_key(&key) {
                return Err(I18nError::Duplicate(key));
            }
            texts.insert(key, text);
        }
        Ok(Self { texts })
    }

    /// The text of `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.texts.get(key).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    /// The keys in order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.texts.keys().map(String::as_str)
    }

    /// The entries in key order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.texts.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Adds the keys of `other`; a key both hold with different texts is a conflict, and then
    /// nothing is added.
    pub fn merge(&mut self, other: &Catalogue) -> Result<(), I18nError> {
        if let Some((key, _)) = other
            .iter()
            .find(|(key, text)| self.get(key).is_some_and(|mine| mine != *text))
        {
            return Err(I18nError::Conflict(key.to_owned()));
        }
        for (key, text) in other.iter() {
            self.texts.insert(key.to_owned(), text.to_owned());
        }
        Ok(())
    }
}

/// Whether `id` can stand as one key segment of `plugin.<id>.*`.
pub(crate) fn is_plugin_id(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// A plugin's `locales/<lang>.json`: at most [`MAX_PLUGIN_LOCALE_BYTES`], UTF-8, and every key
/// below `plugin.<plugin_id>.` - a plugin can never override a core key or another plugin's.
pub fn plugin_catalogue(plugin_id: &str, json: &[u8]) -> Result<Catalogue, I18nError> {
    if !is_plugin_id(plugin_id) {
        return Err(I18nError::BadPluginId(plugin_id.to_owned()));
    }
    if json.len() > MAX_PLUGIN_LOCALE_BYTES {
        return Err(I18nError::TooLarge {
            limit: MAX_PLUGIN_LOCALE_BYTES,
        });
    }
    let text = std::str::from_utf8(json).map_err(|error| I18nError::Json(error.to_string()))?;
    let catalogue = Catalogue::from_json(text)?;
    let prefix = format!("plugin.{plugin_id}.");
    if let Some(key) = catalogue.keys().find(|key| !key.starts_with(&prefix)) {
        return Err(I18nError::ForeignKey {
            plugin: plugin_id.to_owned(),
            key: key.to_owned(),
        });
    }
    Ok(catalogue)
}

/// Reads one JSON object into `out` as (dotted key, text), in file order, duplicates kept.
struct Flatten<'o> {
    prefix: Option<String>,
    out: &'o mut Vec<(String, String)>,
}

impl<'de> DeserializeSeed<'de> for Flatten<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_map(self)
    }
}

impl<'de> Visitor<'de> for Flatten<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object of texts and nested objects")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        while let Some(segment) = map.next_key::<String>()? {
            let key = match &self.prefix {
                Some(prefix) => format!("{prefix}.{segment}"),
                None => segment,
            };
            map.next_value_seed(Entry {
                key,
                out: &mut *self.out,
            })?;
        }
        Ok(())
    }
}

/// One value: a text, or an object nested one segment deeper.
struct Entry<'o> {
    key: String,
    out: &'o mut Vec<(String, String)>,
}

impl<'de> DeserializeSeed<'de> for Entry<'_> {
    type Value = ();

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Entry<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a text or an object at `{}`", self.key)
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<(), E> {
        self.out.push((self.key, text.to_owned()));
        Ok(())
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<(), A::Error> {
        Flatten {
            prefix: Some(self.key),
            out: self.out,
        }
        .visit_map(map)
    }
}

#[cfg(test)]
#[path = "catalogue_tests.rs"]
mod tests;
