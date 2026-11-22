//! Where a settings link opens (RB-090-85): a notice's `settingsKey`, `settings.open { key }`
//! and `openSettings { key }` name a key or an area, and every one of them opens its place.
//!
//! In this order: a registered key opens its field; a category id (`capture`) the category; a
//! section id (`sitemap`) the section, in the first category in navigation order that has it; a
//! key prefix (`visual` for `visual.*`) the one section all its keys sit in, else the one
//! category they share. The registry is the core's keys and the loaded plugins' (RB-094-26,
//! `crate::control::plugin_keys`), so `api-client.benchRepeats` opens its field in Network > API
//! client. The route a section opens at is `settings/<category>?section=<id>`, so the page shows
//! the section's tab and highlights the section. Anything else opens the settings at their start
//! with the link named - `settings?missing=<link>`, or `settings?pending=<link>` while the
//! plugins still load - so the page says which setting it did not find and never shows another
//! place silently (RB-094-26). The same rules check every notice's target in the settings lint
//! (`scripts/lib/settings_targets.py`).
//!
//! A secret key (`x-secret`, RB-094-27) is no place of the window, neither as a key nor under a
//! prefix: its link opens the panel its `x-panel` names instead ([`panel_of`]) -
//! `api-client.secrets` the API client at its Variables tab.

use std::collections::BTreeSet;

use rb_settings::{Category, SECTIONS, SettingsRegistry};

use crate::shell::ROUTE_SETTINGS;

/// The query of a route whose link opens nothing: the page names the link (RB-094-26).
pub const MISSING: &str = "missing";
/// The query of a route whose link opens nothing yet, while the plugins still load: the page
/// asks again once they are in (RB-094-26).
pub const PENDING: &str = "pending";
/// The longest link a route names back.
const MAX_NAMED: usize = 200;

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

/// The panel a link to a secret key opens (RB-094-27): a panel of the key's plugin, at a tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelLink<'a> {
    /// The plugin that registered the key.
    pub plugin: &'a str,
    /// The plugin's panel the key's `x-panel` names.
    pub panel: &'a str,
    /// The tab the panel shows the values in.
    pub tab: Option<&'a str>,
}

/// The panel `link` opens: only a secret key of a plugin whose `x-panel` names one.
pub fn panel_of<'a>(registry: &'a SettingsRegistry, link: &str) -> Option<PanelLink<'a>> {
    let def = registry.get(link)?;
    let plugin = registry.plugin_of(&def.key)?;
    let editor = def.editor_panel()?;
    Some(PanelLink {
        plugin,
        panel: editor.panel,
        tab: editor.tab,
    })
}

/// Where `link` opens in the settings window, `None` for nowhere - a secret key included.
pub fn resolve<'a>(registry: &'a SettingsRegistry, link: &str) -> Option<Target<'a>> {
    if let Some(def) = registry.get(link).filter(|def| !def.is_secret()) {
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
            !def.is_secret()
                && def
                    .key
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
/// `settings/<category>?section=<id>`, or `settings?missing=<link>` where it opens nothing
/// ([`not_found_route`]). Only a registered key, a category id or a section id ever reaches the
/// path.
pub fn route_of(registry: &SettingsRegistry, link: &str) -> String {
    match resolve(registry, link) {
        Some(Target::Key(key)) => format!("{ROUTE_SETTINGS}/{key}"),
        Some(Target::Category(category)) => format!("{ROUTE_SETTINGS}/{}", category.id()),
        Some(Target::Section { category, section }) => {
            format!("{ROUTE_SETTINGS}/{}?section={section}", category.id())
        }
        None => not_found_route(MISSING, link),
    }
}

/// `settings?<query>=<link>` ([`MISSING`], [`PENDING`]) for a link that opens nothing: the
/// settings at their start, and the page names the link. A link of anything but the letters,
/// digits, `.`, `_` and `-` of a key, a category or a section, or longer than a key could be, is
/// never put in the URL: `settings?missing=` then says that the link named no setting.
pub fn not_found_route(query: &str, link: &str) -> String {
    tracing::debug!(
        key = link,
        query,
        "no such setting or area; the settings open at their start and say so"
    );
    let named = if is_nameable(link) { link } else { "" };
    format!("{ROUTE_SETTINGS}?{query}={named}")
}

/// The query of a route whose `link` opens nothing: [`PENDING`] only while the keys of the
/// plugin its first segment names may still come - `installed` says that plugin is there,
/// `loading` that a load runs or is about to (not held back by the first-run wizard) - else
/// [`MISSING`], so the page never waits for keys that do not come.
pub fn not_found_query(
    link: &str,
    installed: impl Fn(&str) -> bool,
    loading: bool,
) -> &'static str {
    let plugin = link.split('.').next().unwrap_or_default();
    if loading && !plugin.is_empty() && installed(plugin) {
        PENDING
    } else {
        MISSING
    }
}

/// Whether `link` may be named back in the URL as it is (nothing in it needs escaping).
fn is_nameable(link: &str) -> bool {
    !link.is_empty()
        && link.len() <= MAX_NAMED
        && link
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "target_walk_tests.rs"]
mod walk_tests;
