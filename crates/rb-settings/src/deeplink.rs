//! Settings deep links: `rbrowser://settings/<key>` opens the settings window at a key, from the
//! command palette, an error message or MCP.
//!
//! The forms: `rbrowser://settings/` (or without the slash) opens the window,
//! `rbrowser://settings/<category id>` a category, `rbrowser://settings/<key>` the key's
//! section with the key in view. A registered key wins over a category of the same name (the key
//! `breakpoints` sits in the category of that name anyway). The scheme and host are matched
//! without regard to case; one trailing `/` is dropped. Nothing is percent-decoded: keys and ids
//! are plain ASCII, so a link with any other character is refused.

use crate::registry::{invalid, is_setting_key};
use crate::{Category, DEEP_LINK_PREFIX, SettingsError, SettingsRegistry};

/// What a deep link opens.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeepLinkTarget {
    /// The settings window at its first category.
    Root,
    /// A category, at its first section.
    Category(Category),
    /// A key: its category and section, with the key in view.
    Key {
        /// The key's category.
        category: Category,
        /// The key's section id.
        section: String,
        /// The key.
        key: String,
    },
}

/// The deep link of `key`: `rbrowser://settings/<key>`.
pub fn deep_link(key: &str) -> String {
    format!("{DEEP_LINK_PREFIX}{key}")
}

impl SettingsRegistry {
    /// What `link` opens. A link with another prefix or a character outside a key is
    /// [`SettingsError::Invalid`]; a well-formed one naming neither a key nor a category is
    /// [`SettingsError::UnknownKey`].
    pub fn resolve_deep_link(&self, link: &str) -> Result<DeepLinkTarget, SettingsError> {
        let path = deep_link_path(link)
            .ok_or_else(|| invalid(link, "not a settings deep link".to_owned()))?;
        if path.is_empty() {
            return Ok(DeepLinkTarget::Root);
        }
        if !is_setting_key(path) {
            return Err(invalid(link, "not a settings key".to_owned()));
        }
        if let Some(def) = self.get(path) {
            return Ok(DeepLinkTarget::Key {
                category: def.category,
                section: def.section.clone(),
                key: def.key.clone(),
            });
        }
        Category::from_id(path)
            .map(DeepLinkTarget::Category)
            .ok_or_else(|| SettingsError::UnknownKey(path.to_owned()))
    }
}

/// The part after the prefix, one trailing `/` dropped; `None` for another prefix.
fn deep_link_path(link: &str) -> Option<&str> {
    let bare = DEEP_LINK_PREFIX.trim_end_matches('/');
    let head = link.get(..bare.len())?;
    if !head.eq_ignore_ascii_case(bare) {
        return None;
    }
    let rest = &link[bare.len()..];
    if rest.is_empty() {
        return Some(rest);
    }
    let path = rest.strip_prefix('/')?;
    Some(path.strip_suffix('/').unwrap_or(path))
}

#[cfg(test)]
#[path = "deeplink_tests.rs"]
mod tests;
