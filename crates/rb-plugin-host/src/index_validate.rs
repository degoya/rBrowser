//! The content rules of [`super::PluginIndex`]: bounds, formats, duplicates.
//!
//! Adapted from rDownloader's `crates/rd-plugin-host/src/index_validate.rs` (RD-140-01).

use std::collections::BTreeSet;

use chrono::Duration;

use super::{
    IndexEntry, IndexError, MAX_INDEX_PLUGINS, MAX_RELEASE_NOTES_CHARS, MAX_VALIDITY_DAYS,
    PLUGIN_INDEX_SCHEMA_VERSION, PluginIndex, Publisher,
};
use crate::manifest::valid_plugin_id;
use crate::package::MAX_PACKAGE_BYTES;

const MAX_NAME_CHARS: usize = 120;
const MAX_DESCRIPTION_CHARS: usize = 1000;
const MAX_KEY_ID_CHARS: usize = 128;
const MAX_LIST_ENTRIES: usize = 64;
const MAX_ENTRY_CHARS: usize = 64;
const MAX_URL_CHARS: usize = 1024;

impl PluginIndex {
    /// Refuses an index this build must not act on.
    ///
    /// # Errors
    ///
    /// [`IndexError::SchemaVersion`] or [`IndexError::Invalid`] with the first rule broken.
    pub fn validate(&self) -> Result<(), IndexError> {
        if self.schema_version != PLUGIN_INDEX_SCHEMA_VERSION {
            return Err(IndexError::SchemaVersion {
                saw: u64::from(self.schema_version),
            });
        }
        if self.sequence == 0 {
            return Err(invalid("the sequence starts at 1"));
        }
        if self.expires_at <= self.issued_at {
            return Err(invalid("expires_at is not after issued_at"));
        }
        if self.expires_at - self.issued_at > Duration::days(MAX_VALIDITY_DAYS) {
            return Err(invalid(format!(
                "valid for longer than {MAX_VALIDITY_DAYS} days"
            )));
        }
        if self.plugins.len() > MAX_INDEX_PLUGINS {
            return Err(invalid(format!("more than {MAX_INDEX_PLUGINS} plugins")));
        }
        let mut seen = BTreeSet::new();
        for entry in &self.plugins {
            let name = format!("{} {}", entry.id, entry.version);
            entry
                .validate()
                .map_err(|reason| invalid(format!("{name}: {reason}")))?;
            if !seen.insert((entry.id.as_str(), entry.version.as_str())) {
                return Err(invalid(format!("{name} is listed twice")));
            }
        }
        Ok(())
    }
}

impl IndexEntry {
    fn validate(&self) -> Result<(), String> {
        if !valid_plugin_id(&self.id) {
            return Err("the id is no plugin id".to_owned());
        }
        bounded_text("name", &self.name, MAX_NAME_CHARS)?;
        if self.name.trim().is_empty() {
            return Err("the name is empty".to_owned());
        }
        bounded_text("description", &self.description, MAX_DESCRIPTION_CHARS)?;
        semver_field("version", &self.version)?;
        semver_field("api_version", &self.api_version)?;
        if let Some(minimum) = &self.min_app_version {
            semver_field("min_app_version", minimum)?;
        }
        if self.kinds.is_empty() {
            return Err("no kind".to_owned());
        }
        word_list("kinds", &self.kinds)?;
        word_list("capabilities", &self.capabilities)?;
        if let Some(category) = &self.category {
            word_list("category", std::slice::from_ref(category))?;
        }
        if !is_sha256_hex(&self.sha256) {
            return Err("sha256 is not 64 lowercase hexadecimal characters".to_owned());
        }
        if self.size == 0 || self.size > MAX_PACKAGE_BYTES {
            return Err(format!(
                "declares an implausible size of {} bytes",
                self.size
            ));
        }
        validate_url(&self.url)?;
        self.publisher.validate()?;
        if let Some(notes) = &self.release_notes {
            if notes.chars().count() > MAX_RELEASE_NOTES_CHARS {
                return Err(format!(
                    "release notes exceed {MAX_RELEASE_NOTES_CHARS} characters"
                ));
            }
            if notes
                .chars()
                .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
            {
                return Err("release notes carry control characters".to_owned());
            }
        }
        Ok(())
    }
}

impl Publisher {
    fn validate(&self) -> Result<(), String> {
        bounded_text("publisher key_id", &self.key_id, MAX_KEY_ID_CHARS)?;
        if self.key_id.trim().is_empty() {
            return Err("publisher key_id is empty".to_owned());
        }
        if !is_sha256_hex(&self.fingerprint) {
            return Err(
                "publisher fingerprint is not 64 lowercase hexadecimal characters".to_owned(),
            );
        }
        bounded_text("publisher author", &self.author, MAX_NAME_CHARS)
    }
}

fn invalid(reason: impl Into<String>) -> IndexError {
    IndexError::Invalid(reason.into())
}

/// Exactly the form `hex_sha256` writes: one spelling per value.
pub(crate) fn is_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Printable text of at most `limit` characters, no control characters.
fn bounded_text(field: &str, value: &str, limit: usize) -> Result<(), String> {
    if value.chars().count() > limit {
        return Err(format!("{field} exceeds {limit} characters"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} carries control characters"));
    }
    Ok(())
}

/// A short list of lowercase words with `-`: kinds and capability ids.
fn word_list(field: &str, list: &[String]) -> Result<(), String> {
    if list.len() > MAX_LIST_ENTRIES {
        return Err(format!("{field} has more than {MAX_LIST_ENTRIES} entries"));
    }
    for entry in list {
        if entry.is_empty()
            || entry.len() > MAX_ENTRY_CHARS
            || !entry
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(format!("{field} holds {entry:?}, which is no name"));
        }
    }
    Ok(())
}

fn semver_field(field: &str, value: &str) -> Result<(), String> {
    bounded_text(field, value, 64)?;
    semver::Version::parse(value)
        .map(|_| ())
        .map_err(|_| format!("{field} {value:?} is not semantic versioning"))
}

/// An absolute `https://` URL, or a relative path that stays below the index's directory: a
/// relative entry is joined onto the index's URL, and refusing `/`, `..`, `\`, a scheme, a query
/// and a fragment means the join can only descend from where the index was found.
fn validate_url(value: &str) -> Result<(), String> {
    if value.is_empty() || value.len() > MAX_URL_CHARS {
        return Err(format!("url is empty or longer than {MAX_URL_CHARS} bytes"));
    }
    if value
        .chars()
        .any(|character| character.is_control() || character == ' ')
    {
        return Err("url carries whitespace or control characters".to_owned());
    }
    if value.starts_with("https://") {
        let parsed = url::Url::parse(value).map_err(|error| format!("url: {error}"))?;
        if parsed.host_str().is_none_or(str::is_empty)
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("url needs a host and no credentials".to_owned());
        }
        return Ok(());
    }
    if value.contains([':', '\\', '?', '#'])
        || value.starts_with('/')
        || value
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        return Err(format!(
            "url {value:?} is neither https:// nor a plain relative path"
        ));
    }
    Ok(())
}
