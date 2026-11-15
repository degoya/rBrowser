//! Every rule a manifest is held to beyond its TOML shape: names, versions, contributions.

use std::collections::BTreeSet;

use anyhow::{Result, bail};
use rb_commands::Group;
use rb_plugin_types::{PluginKind, codes, paths};

use super::{PluginManifest, SUPPORTED_API_VERSIONS};
use crate::refusal::refuse;

/// Ids no plugin may take: they name rBrowser itself or its plugin system - and `checks`, whose
/// out folder (`.rbrowser/out/checks/`) holds the runs of the check runner (RB-060-01).
pub const RESERVED_IDS: &[&str] = &[
    "checks", "core", "plugin", "plugins", "rbrowser", "settings", "system",
];

/// MCP prefixes no plugin may take: the areas of rBrowser's own, unprefixed tools (`views_list`,
/// `settings_get`, `checks_run`, `plugins_list`, ...). RB-030-01 owns the core tool list and may
/// extend this one.
pub const RESERVED_MCP_PREFIXES: &[&str] = &[
    "app",
    "cdp",
    "checks",
    "commands",
    "config",
    "devtools",
    "emulation",
    "env",
    "layout",
    "mcp",
    "navigate",
    "plugin",
    "plugins",
    "reload",
    "settings",
    "shortcuts",
    "sync",
    "trust",
    "view",
    "views",
];

const MAX_NAME_CHARS: usize = 64;
const MAX_DESCRIPTION_CHARS: usize = 500;
const MAX_CONTRIBUTIONS: usize = 64;
const MAX_TOOL_NAME_CHARS: usize = 40;
const RUN_AT: [&str; 3] = ["document-start", "document-end", "document-idle"];

/// Whether `id` is a plugin id: lower-case words of letters and digits joined by single hyphens,
/// starting with a letter, 2 to 48 characters. No dot, so no reverse-DNS form.
#[must_use]
pub fn valid_plugin_id(id: &str) -> bool {
    (2..=48).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

/// Whether `prefix` is an MCP prefix: a lower-case letter, then letters and digits, 2 to 24
/// characters - no `_`, so `<prefix>_<tool>` splits in one way.
#[must_use]
pub fn valid_mcp_prefix(prefix: &str) -> bool {
    (2..=24).contains(&prefix.len())
        && prefix.starts_with(|c: char| c.is_ascii_lowercase())
        && prefix
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// `[a-z][a-z0-9_]*` of at most `max` characters: a command's last segment, a tool's name.
fn valid_segment(text: &str, max: usize) -> bool {
    (1..=max).contains(&text.len())
        && text.starts_with(|c: char| c.is_ascii_lowercase())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `[a-z0-9-]` of 1 to `max` characters: the shape of a panel id, a CLI subcommand and an icon.
fn kebab(text: &str, max: usize) -> bool {
    (1..=max).contains(&text.len())
        && text
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// `[a-z][a-z0-9-]*`: a panel id, a CLI subcommand.
fn valid_word(text: &str) -> bool {
    kebab(text, MAX_TOOL_NAME_CHARS) && text.starts_with(|c: char| c.is_ascii_lowercase())
}

/// The longest panel icon name.
const MAX_ICON_CHARS: usize = 40;

/// `[a-z0-9-]{1,40}`: a panel's icon, a Lucide name (RB-080-61) - never a URL, a path or markup.
fn valid_icon(name: &str) -> bool {
    kebab(name, MAX_ICON_CHARS)
}

/// A language tag of `[locales]`: `en`, `de`, `pt-BR`.
fn valid_language(tag: &str) -> bool {
    let mut parts = tag.split('-');
    let language = parts.next().unwrap_or_default();
    let region = parts.next();
    (2..=3).contains(&language.len())
        && language.chars().all(|c| c.is_ascii_lowercase())
        && region.is_none_or(|region| {
            region.len() == 2 && region.chars().all(|c| c.is_ascii_uppercase())
        })
        && parts.next().is_none()
}

fn single_line(text: &str, max: usize) -> bool {
    !text.trim().is_empty() && text.chars().count() <= max && !text.chars().any(char::is_control)
}

pub(super) fn check(manifest: &PluginManifest) -> Result<()> {
    if !valid_plugin_id(&manifest.id) {
        bail!(
            "id `{}` is no plugin id (lower-case words joined by `-`, 2 to 48 characters)",
            manifest.id
        );
    }
    if RESERVED_IDS.contains(&manifest.id.as_str()) {
        bail!("id `{}` is reserved", manifest.id);
    }
    if !single_line(&manifest.name, MAX_NAME_CHARS) {
        bail!("name must be one line of at most {MAX_NAME_CHARS} characters");
    }
    if manifest.description.chars().count() > MAX_DESCRIPTION_CHARS {
        bail!("description exceeds {MAX_DESCRIPTION_CHARS} characters");
    }
    if !SUPPORTED_API_VERSIONS.contains(&manifest.api_version.as_str()) {
        return Err(refuse(
            codes::API_VERSION,
            format!(
                "api_version {} is not one this rBrowser runs ({})",
                manifest.api_version,
                SUPPORTED_API_VERSIONS.join(", ")
            ),
        ));
    }
    if manifest.kinds.is_empty() {
        bail!("kinds names no plugin kind");
    }
    if !valid_segment(&manifest.key_id, 64) && !valid_word(&manifest.key_id) {
        bail!(
            "key_id `{}` must be lower-case letters, digits, `-` and `_`",
            manifest.key_id
        );
    }
    if manifest.public_key.trim().is_empty() {
        bail!("public_key is empty");
    }
    if let Some(min) = &manifest.metadata.min_app_version {
        semver::Version::parse(min)
            .map_err(|error| anyhow::anyhow!("metadata.min_app_version `{min}`: {error}"))?;
    }
    manifest.limits.check().map_err(anyhow::Error::msg)?;
    check_prefix(manifest)?;
    check_locales(manifest)?;
    check_contributions(manifest)
}

fn check_prefix(manifest: &PluginManifest) -> Result<()> {
    match &manifest.mcp_prefix {
        Some(prefix) if !valid_mcp_prefix(prefix) => {
            bail!(
                "mcp_prefix `{prefix}` must be a lower-case letter, then letters and digits (2 to 24)"
            )
        }
        Some(prefix) if RESERVED_MCP_PREFIXES.contains(&prefix.as_str()) => Err(refuse(
            codes::MCP_PREFIX_RESERVED,
            format!("mcp_prefix `{prefix}` is an area of rBrowser's own MCP tools"),
        )),
        None if !manifest.tools.is_empty() => bail!("a plugin with [[tools]] needs an mcp_prefix"),
        _ => Ok(()),
    }
}

fn check_locales(manifest: &PluginManifest) -> Result<()> {
    if !manifest.locales.contains_key("en") {
        bail!("[locales] needs `en`");
    }
    for (language, member) in &manifest.locales {
        if !valid_language(language) {
            bail!("[locales] `{language}` is no language tag");
        }
        if *member != format!("locales/{language}.json") {
            bail!("[locales] {language} must be \"locales/{language}.json\"");
        }
    }
    Ok(())
}

fn check_key(manifest: &PluginManifest, field: &str, key: &str) -> Result<()> {
    if !key.starts_with(&manifest.key_prefix()) || !rb_i18n::is_well_formed_key(key) {
        bail!(
            "{field} `{key}` must be an i18n key below `{}`",
            manifest.key_prefix()
        );
    }
    Ok(())
}

fn check_contributions(manifest: &PluginManifest) -> Result<()> {
    let contributes = !manifest.commands.is_empty()
        || !manifest.tools.is_empty()
        || !manifest.page_scripts.is_empty()
        || !manifest.panels.is_empty()
        || !manifest.cli.is_empty();
    if contributes && !manifest.is(PluginKind::Feature) {
        bail!("commands, tools, page scripts, panels and cli entries need kind feature-plugin");
    }
    for (what, count) in [
        ("commands", manifest.commands.len()),
        ("tools", manifest.tools.len()),
        ("page_scripts", manifest.page_scripts.len()),
        ("panels", manifest.panels.len()),
        ("cli", manifest.cli.len()),
    ] {
        if count > MAX_CONTRIBUTIONS {
            bail!("more than {MAX_CONTRIBUTIONS} {what}");
        }
    }
    let mut tools = BTreeSet::new();
    for tool in &manifest.tools {
        if !valid_segment(&tool.name, MAX_TOOL_NAME_CHARS) || !tools.insert(tool.name.as_str()) {
            bail!("tools: `{}` is no tool name or given twice", tool.name);
        }
        check_key(manifest, "tools.description", &tool.description)?;
        if let Some(input) = &tool.input
            && input.get("type").and_then(toml::Value::as_str) != Some("object")
        {
            bail!(
                "tools.{}.input must be a JSON schema of type \"object\"",
                tool.name
            );
        }
    }
    let mut commands = BTreeSet::new();
    for command in &manifest.commands {
        if !valid_segment(&command.id, MAX_TOOL_NAME_CHARS) || !commands.insert(command.id.as_str())
        {
            bail!("commands: `{}` is no command id or given twice", command.id);
        }
        check_key(manifest, "commands.title", &command.title)?;
        if let Some(group) = &command.group
            && serde_json::from_value::<Group>(serde_json::Value::String(group.clone())).is_err()
        {
            bail!("commands.{}.group `{group}` is no leader group", command.id);
        }
        if command
            .shortcut
            .as_ref()
            .is_some_and(|text| !single_line(text, 64))
        {
            bail!("commands.{}.shortcut is malformed", command.id);
        }
        match (&command.tool, &command.mcp_omitted) {
            (Some(tool), None) if tools.contains(tool.as_str()) => {}
            (Some(tool), None) => {
                bail!("commands.{}.tool `{tool}` is no declared tool", command.id)
            }
            (None, Some(reason)) if single_line(reason, 200) => {}
            _ => bail!(
                "commands.{} needs exactly one of `tool` and `mcp_omitted`",
                command.id
            ),
        }
    }
    for script in &manifest.page_scripts {
        check_member(&script.path, "page/", ".js")?;
        if !RUN_AT.contains(&script.run_at.as_str()) {
            bail!("page_scripts.run_at `{}` is unknown", script.run_at);
        }
    }
    let mut panels = BTreeSet::new();
    for panel in &manifest.panels {
        if !valid_word(&panel.id) || !panels.insert(panel.id.as_str()) {
            bail!("panels: `{}` is no panel id or given twice", panel.id);
        }
        check_key(manifest, "panels.title", &panel.title)?;
        check_member(&panel.entry, "panel/", ".html")?;
        if let Some(icon) = &panel.icon
            && !valid_icon(icon)
        {
            bail!(
                "panels.{}.icon is no icon name (a Lucide name, `[a-z0-9-]`, 1 to {MAX_ICON_CHARS} characters)",
                panel.id
            );
        }
    }
    let mut cli = BTreeSet::new();
    for entry in &manifest.cli {
        if !valid_word(&entry.name) || !cli.insert(entry.name.as_str()) {
            bail!("cli: `{}` is no subcommand name or given twice", entry.name);
        }
        if !commands.contains(entry.command.as_str()) {
            bail!(
                "cli.{}.command `{}` is no declared command",
                entry.name,
                entry.command
            );
        }
    }
    let mut menu = BTreeSet::new();
    for command in &manifest.context_menu {
        if !commands.contains(command.as_str()) {
            bail!("context_menu names `{command}`, which is no declared command");
        }
        if !menu.insert(command.as_str()) {
            bail!("context_menu names `{command}` twice");
        }
    }
    Ok(())
}

fn check_member(path: &str, folder: &str, extension: &str) -> Result<()> {
    if !path.starts_with(folder)
        || !path.ends_with(extension)
        || path.contains('\\')
        || paths::relative_segments(path).is_err()
    {
        bail!("`{path}` must be a package member under `{folder}` ending in `{extension}`");
    }
    Ok(())
}
