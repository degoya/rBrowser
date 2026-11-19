//! Where a settings link opens (RB-090-85): a notice's `settingsKey`, `settings.open { key }`
//! and `openSettings { key }` name a key or an area, and every one of them opens its place.
//!
//! In this order: a registered key opens its field; a category id (`capture`) the category; a
//! section id (`sitemap`) the section, in the first category in navigation order that has it; a
//! key prefix (`visual` for `visual.*`) the one section all its keys sit in, else the one
//! category they share. Anything else opens the settings at their start. The route a section
//! opens at is `settings/<category>?section=<id>`, so the page shows the section's tab and
//! highlights the section. The same rules check every notice's target in the settings lint
//! (`scripts/lib/settings_targets.py`).

use std::collections::BTreeSet;

use rb_settings::{Category, SECTIONS, SettingsRegistry};

use crate::shell::ROUTE_SETTINGS;

/// The place a settings link opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target<'a> {
    /// A key: its field, highlighted.
    Key(&'a str),
    /// A category, at its first section (or the tab it was left on).
    Category(Category),
    /// A section of a category, highlighted.
    Section {
        /// The category the section belongs to.
        category: Category,
        /// The section id.
        section: &'static str,
    },
}

/// Where `link` opens, `None` for nowhere.
pub fn resolve<'a>(registry: &'a SettingsRegistry, link: &str) -> Option<Target<'a>> {
    if let Some(def) = registry.get(link) {
        return Some(Target::Key(&def.key));
    }
    if let Some(category) = Category::from_id(link) {
        return Some(Target::Category(category));
    }
    if let Some(section) = SECTIONS.iter().find(|section| section.id == link) {
        return Some(Target::Section {
            category: section.category,
            section: section.id,
        });
    }
    prefix_target(registry, link)
}

/// The one section, else the one category, the keys under `prefix.` share.
fn prefix_target<'a>(registry: &SettingsRegistry, prefix: &str) -> Option<Target<'a>> {
    if prefix.is_empty() {
        return None;
    }
    let places: BTreeSet<(Category, &str)> = registry
        .iter()
        .filter(|def| {
            def.key
                .strip_prefix(prefix)
                .is_some_and(|rest| rest.starts_with('.'))
        })
        .map(|def| (def.category, def.section.as_str()))
        .collect();
    let categories: BTreeSet<Category> = places.iter().map(|(category, _)| *category).collect();
    match (places.len(), categories.len()) {
        (1, _) => {
            let (category, id) = places.into_iter().next()?;
            let section = rb_settings::section(category, id)?;
            Some(Target::Section {
                category,
                section: section.id,
            })
        }
        (_, 1) => categories.into_iter().next().map(Target::Category),
        _ => None,
    }
}

/// The pages window's route for `link`: `settings/<key>`, `settings/<category>`,
/// `settings/<category>?section=<id>`, or `settings` where it opens nothing. Only a registered
/// key, a category id or a section id ever reaches the URL.
pub fn route_of(registry: &SettingsRegistry, link: &str) -> String {
    match resolve(registry, link) {
        Some(Target::Key(key)) => format!("{ROUTE_SETTINGS}/{key}"),
        Some(Target::Category(category)) => format!("{ROUTE_SETTINGS}/{}", category.id()),
        Some(Target::Section { category, section }) => {
            format!("{ROUTE_SETTINGS}/{}?section={section}", category.id())
        }
        None => {
            tracing::debug!(
                key = link,
                "no such setting or area; the settings open at their start"
            );
            ROUTE_SETTINGS.to_owned()
        }
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
