//! Validation of the merged config: first against the types (the schema is generated from them,
//! so a value the types refuse breaks the schema), then the rules a schema cannot say - unique
//! ids, references between sections, cells inside the grid, the emulation extras (RB-080-10),
//! `http`/`https` only for `startUrl` and the environments' URLs, bare `http` origins for their
//! `assetOrigins` (RB-090-65), the fields of the layout modes
//! (RB-080-04, `layout_modes`).
//!
//! Every problem is a [`ConfigIssue`] with a JSON pointer into the merged config and an i18n key
//! under `config.error.*`. The type check stops at its first problem; the rules report all.

use std::collections::HashSet;

use rb_util::json_pointer::escape;
use serde_json::Value;

use crate::bookmarks::is_web_url;
use crate::emulation_extras::check_extras;
use crate::{ConfigError, ConfigIssue, Layout, ProjectConfig, Zoom};

/// The session every Chromium view without one shares; it need not be listed in `sessions`.
pub const DEFAULT_SESSION: &str = "default";
/// The i18n key of a `startUrl` that is no absolute `http`/`https` URL.
pub const START_URL_KEY: &str = "config.error.start_url";
/// The i18n key of an `environments[].url` that is no absolute `http`/`https` URL.
pub const ENVIRONMENT_URL_KEY: &str = "config.error.environment_url";
/// The i18n key of an `environments[].assetOrigins` entry that is no bare `http` origin.
pub const ASSET_ORIGIN_KEY: &str = "config.error.asset_origin";

/// Deserializes and checks the merged config value.
pub fn validate(value: Value) -> Result<ProjectConfig, ConfigError> {
    let config: ProjectConfig = serde_path_to_error::deserialize(value).map_err(|error| {
        ConfigError::Invalid(vec![issue(
            pointer_of(error.path()),
            "config.error.invalid_value",
            error.inner().to_string(),
        )])
    })?;
    let issues = check(&config);
    if issues.is_empty() {
        Ok(config)
    } else {
        Err(ConfigError::Invalid(issues))
    }
}

/// The rules beyond the types, every problem at once.
pub fn check(config: &ProjectConfig) -> Vec<ConfigIssue> {
    let mut issues = Vec::new();
    let breakpoint_ids = unique_ids(
        config.breakpoints.iter().map(|b| b.id.as_str()),
        "/breakpoints",
        &mut issues,
    );
    unique_ids(
        config.environments.iter().map(|e| e.id.as_str()),
        "/environments",
        &mut issues,
    );
    check_web_urls(config, &mut issues);
    for (index, breakpoint) in config.breakpoints.iter().enumerate() {
        let at = format!("/breakpoints/{index}");
        for (field, size) in [("width", breakpoint.width), ("height", breakpoint.height)] {
            match size {
                Some(0) => issues.push(issue(
                    format!("{at}/{field}"),
                    "config.error.size_zero",
                    format!("breakpoint `{}`: {field} must be at least 1", breakpoint.id),
                )),
                None if breakpoint.device.is_none() && breakpoint.emulation.paper.is_none() => {
                    issues.push(issue(
                        format!("{at}/{field}"),
                        "config.error.size_required",
                        format!(
                            "breakpoint `{}` has no {field}: set `{field}` in CSS px, or \
                             `device` to a device preset whose viewport supplies it",
                            breakpoint.id
                        ),
                    ));
                }
                _ => {}
            }
        }
        issues.extend(check_extras(&breakpoint.emulation, &at));
        if let Some(color) = &breakpoint.color
            && crate::parse_hex_color(color).is_none()
        {
            issues.push(issue(
                format!("{at}/color"),
                "config.error.breakpoint_color",
                format!("breakpoint `{}`: the colour is `#rrggbb`", breakpoint.id),
            ));
        }
        if let Some(session) = &breakpoint.session
            && session != DEFAULT_SESSION
            && !config.sessions.contains(session)
        {
            issues.push(issue(
                format!("{at}/session"),
                "config.error.unknown_session",
                format!("session `{session}` is not listed in `sessions`"),
            ));
        }
    }
    issues.extend(crate::sessions::check_sessions(config));
    issues.extend(check_extras(&config.emulation, "/emulation"));
    issues.extend(crate::bookmarks::check_bookmarks(config));
    check_layout(&config.layout, "/layout", &breakpoint_ids, &mut issues);
    issues.extend(crate::layout_modes::check_hidden(
        &config.layout.hidden,
        "/layout/hidden",
        &breakpoint_ids,
    ));
    for (mode, fields) in config.layout.modes.iter() {
        issues.extend(crate::layout_modes::check_modes(
            fields,
            &format!("/layout/modes/{mode}"),
            &breakpoint_ids,
        ));
    }
    if let Some(default) = &config.layouts.default
        && !config.layouts.presets.contains_key(default)
    {
        issues.push(issue(
            "/layouts/default".to_owned(),
            "config.error.unknown_preset",
            format!("layout preset `{default}` does not exist"),
        ));
    }
    for (name, preset) in &config.layouts.presets {
        let at = format!("/layouts/presets/{}", escape(name));
        if let Some(layout) = &preset.layout {
            check_layout(
                layout,
                &format!("{at}/layout"),
                &breakpoint_ids,
                &mut issues,
            );
        }
        if let Some(focus) = &preset.focus
            && !breakpoint_ids.contains(focus.as_str())
        {
            issues.push(issue(
                format!("{at}/focus"),
                "config.error.unknown_breakpoint",
                format!("breakpoint `{focus}` does not exist"),
            ));
        }
        if let Some(hidden) = &preset.hidden {
            issues.extend(crate::layout_modes::check_hidden(
                hidden,
                &format!("{at}/hidden"),
                &breakpoint_ids,
            ));
        }
        issues.extend(crate::layout_modes::check_modes(
            &preset.modes,
            &at,
            &breakpoint_ids,
        ));
    }
    issues
}

/// Reports every repeated id of `ids` (at `<at>/<index>/id`) and returns the set of ids.
fn unique_ids<'a>(
    ids: impl Iterator<Item = &'a str>,
    at: &str,
    issues: &mut Vec<ConfigIssue>,
) -> HashSet<&'a str> {
    let mut seen = HashSet::new();
    for (index, id) in ids.enumerate() {
        if !seen.insert(id) {
            issues.push(issue(
                format!("{at}/{index}/id"),
                "config.error.duplicate_id",
                format!("id `{id}` is used more than once"),
            ));
        }
    }
    seen
}

fn check_layout(layout: &Layout, at: &str, ids: &HashSet<&str>, issues: &mut Vec<ConfigIssue>) {
    for (field, size) in [("columns", layout.columns), ("rows", layout.rows)] {
        if size == 0 {
            issues.push(issue(
                format!("{at}/{field}"),
                "config.error.grid_size",
                format!("the grid needs at least one of {field}"),
            ));
        }
    }
    for (index, cell) in layout.cells.iter().enumerate() {
        let cell_at = format!("{at}/cells/{index}");
        if !ids.contains(cell.breakpoint.as_str()) {
            issues.push(issue(
                format!("{cell_at}/breakpoint"),
                "config.error.unknown_breakpoint",
                format!("breakpoint `{}` does not exist", cell.breakpoint),
            ));
        }
        let last_col = u64::from(cell.col) + u64::from(cell.col_span);
        let last_row = u64::from(cell.row) + u64::from(cell.row_span);
        if cell.col == 0
            || cell.row == 0
            || cell.col_span == 0
            || cell.row_span == 0
            || last_col - 1 > u64::from(layout.columns)
            || last_row - 1 > u64::from(layout.rows)
        {
            issues.push(issue(
                cell_at.clone(),
                "config.error.cell_outside_grid",
                format!(
                    "cell at column {} row {} (span {} x {}) lies outside the {} x {} grid",
                    cell.col, cell.row, cell.col_span, cell.row_span, layout.columns, layout.rows
                ),
            ));
        }
        if let Zoom::Factor(factor) = cell.zoom
            && !(0.25..=1.0).contains(&factor)
        {
            issues.push(issue(
                format!("{cell_at}/zoom"),
                "config.error.zoom_range",
                format!("zoom {factor} is outside 0.25 to 1"),
            ));
        }
    }
}

/// `startUrl` and every `environments[].url` open in all views: an untrusted repository may not
/// point them at `file:`, `data:`, `javascript:` or any other scheme but `http`/`https`.
fn check_web_urls(config: &ProjectConfig, issues: &mut Vec<ConfigIssue>) {
    if let Some(url) = &config.start_url
        && !is_web_url(url)
    {
        issues.push(issue(
            "/startUrl".to_owned(),
            START_URL_KEY,
            "startUrl must be an absolute http or https URL".to_owned(),
        ));
    }
    for (index, environment) in config.environments.iter().enumerate() {
        if !is_web_url(&environment.url) {
            issues.push(issue(
                format!("/environments/{index}/url"),
                ENVIRONMENT_URL_KEY,
                format!(
                    "environment `{}`: url must be an absolute http or https URL",
                    environment.id
                ),
            ));
        }
        for (entry, origin) in environment.asset_origins.iter().enumerate() {
            if !is_http_origin(origin) {
                issues.push(issue(
                    format!("/environments/{index}/assetOrigins/{entry}"),
                    ASSET_ORIGIN_KEY,
                    format!(
                        "environment `{}`: an asset origin must be http://host[:port] and \
                         nothing more",
                        environment.id
                    ),
                ));
            }
        }
    }
}

/// Whether `value` is a bare `http` origin, as `environments[].assetOrigins` takes it:
/// `http://host[:port]`, at most a trailing `/`, no user info, path, query or fragment
/// (RB-090-65). The portal forwards to it, so nothing about it may be left to guess.
pub fn is_http_origin(value: &str) -> bool {
    let Some(rest) = value
        .get(..7)
        .filter(|scheme| scheme.eq_ignore_ascii_case("http://"))
        .and_then(|_| value.get(7..))
    else {
        return false;
    };
    let authority = rest.strip_suffix('/').unwrap_or(rest);
    is_web_url(value) && !authority.is_empty() && !authority.contains(['/', '?', '#', '@'])
}

fn issue(path: String, key: &str, message: String) -> ConfigIssue {
    ConfigIssue {
        path,
        key: key.to_owned(),
        message,
    }
}

/// The JSON pointer of a deserializer path; an untracked segment (inside a flattened struct)
/// ends it at the last known level.
pub(crate) fn pointer_of(path: &serde_path_to_error::Path) -> String {
    use serde_path_to_error::Segment;
    let mut pointer = String::new();
    for segment in path.iter() {
        match segment {
            Segment::Seq { index } => pointer.push_str(&format!("/{index}")),
            Segment::Map { key } => {
                pointer.push('/');
                pointer.push_str(&escape(key));
            }
            Segment::Enum { .. } => {}
            Segment::Unknown => break,
        }
    }
    pointer
}

#[cfg(test)]
#[path = "validate_tests.rs"]
mod tests;
