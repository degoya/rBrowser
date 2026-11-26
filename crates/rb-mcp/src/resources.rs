//! The MCP resources (RB-095-01): rBrowser's state to read without a tool call, read-only.
//!
//! - `rbrowser://views`: what `views_list` answers (views, breakpoints, sizes, URLs).
//! - `rbrowser://findings/latest`: the last check run (RB-060-01) with its message texts,
//!   masked.
//! - `rbrowser://screenshots/latest`: the last screenshots saved (RB-050-03) - paths, sizes and
//!   per file a URI; the image itself only when that URI is read
//!   (`rbrowser://screenshots/latest/{index}`, a PNG blob). The index picks one of the files the
//!   host recorded; it never names a path.
//! - `rbrowser://config`: what `config_get` answers, secrets masked, every `x-protected` key left
//!   out ([`crate::protected_keys`]).
//!
//! The same rights as the tools: the request carries the token, nothing here writes, trusts or
//! installs, and every answer is held to `mcp.maxAnswerBytes` (refused over it, as the
//! screenshot tool's inline PNGs are). With `mcp.resources` off none is listed or read.

use base64::Engine as _;
use rmcp::model::{ReadResourceResult, Resource, ResourceContents, ResourceTemplate};
use serde_json::Value;

use crate::RbMcpServer;
use crate::backend::ToolFailure;

/// The setting that offers the resources.
pub const SETTING: &str = "mcp.resources";
/// The views of the grid.
pub const VIEWS: &str = "rbrowser://views";
/// The last check run.
pub const FINDINGS: &str = "rbrowser://findings/latest";
/// The last screenshots saved.
pub const SCREENSHOTS: &str = "rbrowser://screenshots/latest";
/// One file of [`SCREENSHOTS`], the PNG itself.
pub const SCREENSHOT_TEMPLATE: &str = "rbrowser://screenshots/latest/{index}";
/// The project config.
pub const CONFIG: &str = "rbrowser://config";

/// The code of a resource read while `mcp.resources` is off.
pub const DISABLED: &str = "resources.disabled";
/// The code of a URI no resource has.
pub const NOT_FOUND: &str = "resources.not_found";
/// The code of `rbrowser://findings/latest` before the first check run.
pub const NO_FINDINGS: &str = "findings.none";
/// The code of `rbrowser://screenshots/latest` before the first screenshot saved, and of an
/// index past its files.
pub const NO_SCREENSHOTS: &str = "screenshots.none";
/// The code of an answer over `mcp.maxAnswerBytes`.
pub const TOO_LARGE: &str = "answer.too_large";

const JSON: &str = "application/json";
const PNG: &str = "image/png";
/// The most digits of a screenshot's index.
const MAX_INDEX_DIGITS: usize = 4;

/// The resources in the order `resources/list` gives them: URI, name, title, description.
const RESOURCES: [(&str, &str, &str, &str); 4] = [
    (
        VIEWS,
        "views",
        "Views",
        "The views of the grid in grid order, as views_list answers them: id, label, \
         breakpoint, engine, width, height, dpr, URL, title, loading state, sync state.",
    ),
    (
        FINDINGS,
        "findings-latest",
        "Latest findings",
        "The last check run (checks_run or the findings panel): its findings merged across views \
         and plugins with severity, rule, elements and breakpoints, the summary, and the texts \
         of its message keys; secrets masked.",
    ),
    (
        SCREENSHOTS,
        "screenshots-latest",
        "Latest screenshots",
        "The last screenshots saved to the project's screenshot folder: per view label, size, \
         path and the uri of the PNG (rbrowser://screenshots/latest/<index>), read only when \
         the image is needed.",
    ),
    (
        CONFIG,
        "config",
        "Project config",
        "The effective project config (.rbrowser/config.json merged with config.local.json) as \
         config_get answers it, secrets masked and every protected key left out (breakpoints \
         are in rbrowser://views, request headers and the network keys nowhere).",
    ),
];

/// Every resource.
#[must_use]
pub fn list() -> Vec<Resource> {
    RESOURCES
        .iter()
        .map(|(uri, name, title, description)| {
            Resource::new(*uri, *name)
                .with_title(*title)
                .with_description(*description)
                .with_mime_type(JSON)
        })
        .collect()
}

/// The screenshot files' template.
#[must_use]
pub fn templates() -> Vec<ResourceTemplate> {
    vec![
        ResourceTemplate::new(SCREENSHOT_TEMPLATE, "screenshot")
            .with_title("Screenshot")
            .with_description(
                "One PNG of rbrowser://screenshots/latest as a blob: index counts the files \
                 listed there, the overview last. At most mcp.maxAnswerBytes of base64.",
            )
            .with_mime_type(PNG),
    ]
}

/// What a URI names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Named {
    Views,
    Findings,
    Screenshots,
    Screenshot(usize),
    Config,
}

/// The resource `uri` names, exactly as listed (no case folding, no path of its own).
fn parse(uri: &str) -> Option<Named> {
    match uri {
        VIEWS => Some(Named::Views),
        FINDINGS => Some(Named::Findings),
        SCREENSHOTS => Some(Named::Screenshots),
        CONFIG => Some(Named::Config),
        _ => {
            let index = uri.strip_prefix(SCREENSHOTS)?.strip_prefix('/')?;
            let digits = !index.is_empty()
                && index.len() <= MAX_INDEX_DIGITS
                && index.bytes().all(|byte| byte.is_ascii_digit());
            if !digits {
                return None;
            }
            index.parse().ok().map(Named::Screenshot)
        }
    }
}

/// The URI of screenshot file `index`.
fn screenshot_uri(index: usize) -> String {
    format!("{SCREENSHOTS}/{index}")
}

/// The host's record of the last screenshots with a `uri` for each file - the shots with a
/// `path` in their order, then the overview: the order of
/// [`crate::Backend::latest_screenshot_png`] - and no image data.
#[must_use]
pub fn with_uris(mut listed: Value) -> Value {
    let mut index = 0;
    let mut mark = |shot: &mut Value| {
        if let Value::Object(map) = shot {
            map.remove("pngBase64");
            if map.get("path").is_some_and(Value::is_string) {
                map.insert("uri".to_owned(), Value::String(screenshot_uri(index)));
                index += 1;
            }
        }
    };
    if let Some(shots) = listed.get_mut("shots").and_then(Value::as_array_mut) {
        shots.iter_mut().for_each(&mut mark);
    }
    if let Some(overview) = listed.get_mut("overview") {
        mark(overview);
    }
    listed
}

/// `value` as the JSON content of `uri`, at most `limit` bytes.
fn json_content(uri: &str, value: &Value, limit: usize) -> Result<ReadResourceResult, ToolFailure> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| ToolFailure::new("mcp.internal", error.to_string()))?;
    if text.len() > limit {
        return Err(too_large(text.len(), limit));
    }
    Ok(ReadResourceResult::new(vec![
        ResourceContents::text(text, uri).with_mime_type(JSON),
    ]))
}

fn too_large(bytes: usize, limit: usize) -> ToolFailure {
    ToolFailure::new(
        TOO_LARGE,
        format!(
            "the resource has {bytes} bytes, more than mcp.maxAnswerBytes ({limit}); read it with \
             its tool (views_list, checks_export, screenshot, config_get) instead"
        ),
    )
    .with_setting("mcp.maxAnswerBytes")
}

impl RbMcpServer {
    /// `resources/list`: every resource while `mcp.resources` is on, else none.
    pub(crate) fn resource_list(&self) -> Vec<Resource> {
        if self.backend.options().resources {
            list()
        } else {
            Vec::new()
        }
    }

    /// `resources/templates/list`: the screenshot template while `mcp.resources` is on.
    pub(crate) fn resource_templates(&self) -> Vec<ResourceTemplate> {
        if self.backend.options().resources {
            templates()
        } else {
            Vec::new()
        }
    }

    /// `resources/read` of `uri` while `mcp.resources` is on.
    pub(crate) async fn resource_read(&self, uri: &str) -> Result<ReadResourceResult, ToolFailure> {
        let options = self.backend.options();
        if !options.resources {
            return Err(ToolFailure::new(
                DISABLED,
                "the resources are off: a person switches them on in Settings > Automation > \
                 MCP (rbrowser://settings/mcp.resources)",
            )
            .with_setting(SETTING));
        }
        let named = parse(uri).ok_or_else(|| {
            ToolFailure::new(
                NOT_FOUND,
                format!("no resource `{}`; resources/list names them", cut(uri)),
            )
        })?;
        let limit = options.max_answer_bytes;
        let value = match named {
            Named::Views => self.read(|backend| backend.views_list()).await?,
            Named::Findings => self
                .read(|backend| backend.latest_findings().map(|run| backend.mask(&run)))
                .await?
                .ok_or_else(|| {
                    ToolFailure::new(NO_FINDINGS, "no check run yet: run checks_run first")
                })?,
            Named::Screenshots => self
                .read(|backend| backend.latest_screenshots())
                .await?
                .map(with_uris)
                .ok_or_else(no_screenshots)?,
            Named::Config => {
                let config = self.read(|backend| backend.config()).await?;
                crate::protected_keys::without(config, crate::protected_keys::paths())
            }
            Named::Screenshot(index) => return self.screenshot_blob(uri, index, limit).await,
        };
        json_content(uri, &value, limit)
    }

    /// The PNG of screenshot file `index` as a base64 blob of at most `limit` bytes.
    async fn screenshot_blob(
        &self,
        uri: &str,
        index: usize,
        limit: usize,
    ) -> Result<ReadResourceResult, ToolFailure> {
        // Base64 makes four bytes of three.
        let raw_limit = limit / 4 * 3;
        let png = self
            .read(move |backend| backend.latest_screenshot_png(index, raw_limit))
            .await??;
        let blob = base64::engine::general_purpose::STANDARD.encode(png);
        if blob.len() > limit {
            return Err(too_large(blob.len(), limit));
        }
        Ok(ReadResourceResult::new(vec![
            ResourceContents::blob(blob, uri).with_mime_type(PNG),
        ]))
    }
}

fn no_screenshots() -> ToolFailure {
    ToolFailure::new(
        NO_SCREENSHOTS,
        "no screenshot was saved yet: take one with screenshot",
    )
}

/// At most 200 characters of `uri` for a message.
fn cut(uri: &str) -> String {
    rb_util::text::cut_ellipsis(uri, 200)
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
