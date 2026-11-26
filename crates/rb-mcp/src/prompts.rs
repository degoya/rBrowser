//! The MCP prompts (RB-095-01): the workflows rBrowser foresees, offered to an agent's client as
//! ready texts - `responsive_review` (every breakpoint looked at, findings reported),
//! `a11y_triage` (axe's findings ordered by severity, fixes proposed) and `bug_repro` (a bug
//! reproduced in every view, then `bug_report`, RB-093-04). Each takes `url` (loaded in every
//! view first) and `breakpoints` (ids, separated by commas). The texts are the catalogue's
//! (`mcp.prompt.*` in `locales/en.json`); here only their names, their arguments and the checks
//! of what a client fills in. A prompt runs nothing: it is text, the agent calls the tools with
//! their own rights. With `mcp.prompts` off none is listed or answered.

use rmcp::model::{GetPromptResult, JsonObject, Prompt, PromptArgument, PromptMessage, Role};
use serde_json::Value;

use crate::RbMcpServer;
use crate::backend::ToolFailure;

/// The setting that offers the prompts.
pub const SETTING: &str = "mcp.prompts";
/// The code of a prompt asked for while `mcp.prompts` is off.
pub const DISABLED: &str = "prompts.disabled";
/// The code of a name no prompt has.
pub const NOT_FOUND: &str = "prompts.not_found";
/// The code of an argument the prompts do not take, or a value they refuse.
pub const INVALID: &str = "args.invalid";
/// The most characters `url` may have.
pub const MAX_URL_CHARS: usize = 2048;
/// The most breakpoint ids `breakpoints` may name.
pub const MAX_BREAKPOINTS: usize = 32;
/// The most characters of one breakpoint id.
const MAX_ID_CHARS: usize = 64;

/// The prompts' names, in the order `prompts/list` gives them.
pub const NAMES: [&str; 3] = ["responsive_review", "a11y_triage", "bug_repro"];

/// One prompt: its name and the catalogue keys of its title, description and text.
struct Def {
    name: &'static str,
    title: &'static str,
    description: &'static str,
    text: &'static str,
}

const PROMPTS: [Def; 3] = [
    Def {
        name: NAMES[0],
        title: "mcp.prompt.responsive_review.title",
        description: "mcp.prompt.responsive_review.description",
        text: "mcp.prompt.responsive_review.text",
    },
    Def {
        name: NAMES[1],
        title: "mcp.prompt.a11y_triage.title",
        description: "mcp.prompt.a11y_triage.description",
        text: "mcp.prompt.a11y_triage.text",
    },
    Def {
        name: NAMES[2],
        title: "mcp.prompt.bug_repro.title",
        description: "mcp.prompt.bug_repro.description",
        text: "mcp.prompt.bug_repro.text",
    },
];

/// The arguments every prompt takes, with the catalogue keys of their descriptions.
const ARGUMENTS: [(&str, &str); 2] = [
    ("url", "mcp.prompt.arg.url"),
    ("breakpoints", "mcp.prompt.arg.breakpoints"),
];

/// Every prompt with its title, description and arguments (none required).
#[must_use]
pub fn list() -> Vec<Prompt> {
    PROMPTS
        .iter()
        .map(|def| {
            Prompt::new(
                def.name,
                Some(rb_i18n::t(def.description)),
                Some(arguments()),
            )
            .with_title(rb_i18n::t(def.title))
        })
        .collect()
}

fn arguments() -> Vec<PromptArgument> {
    ARGUMENTS
        .iter()
        .map(|(name, key)| {
            PromptArgument::new(*name)
                .with_description(rb_i18n::t(key))
                .with_required(false)
        })
        .collect()
}

/// The prompt `name` with `arguments` filled in: one user message.
///
/// # Errors
///
/// An unknown name ([`NOT_FOUND`]), an argument the prompts do not take or a value they refuse
/// ([`INVALID`]).
pub fn get(name: &str, arguments: Option<&JsonObject>) -> Result<GetPromptResult, ToolFailure> {
    let def = PROMPTS.iter().find(|def| def.name == name).ok_or_else(|| {
        ToolFailure::new(
            NOT_FOUND,
            format!("no prompt `{name}`; prompts/list names them"),
        )
    })?;
    let args = Args::parse(arguments)?;
    let start = match &args.url {
        Some(url) => rb_i18n::t_with("mcp.prompt.start.url", &[("url", url)]),
        None => rb_i18n::t("mcp.prompt.start.current"),
    };
    let scope = match &args.breakpoints {
        Some(ids) => rb_i18n::t_with(
            "mcp.prompt.scope.breakpoints",
            &[("breakpoints", &ids.join(", "))],
        ),
        None => rb_i18n::t("mcp.prompt.scope.all"),
    };
    let text = rb_i18n::t_with(def.text, &[("start", &start), ("scope", &scope)]);
    Ok(
        GetPromptResult::new(vec![PromptMessage::new_text(Role::User, text)])
            .with_description(rb_i18n::t(def.description)),
    )
}

/// The arguments a client filled in, checked.
#[derive(Debug, Default)]
struct Args {
    url: Option<String>,
    breakpoints: Option<Vec<String>>,
}

impl Args {
    /// Every argument is a string; an empty one counts as absent.
    fn parse(arguments: Option<&JsonObject>) -> Result<Self, ToolFailure> {
        let mut args = Self::default();
        for (name, value) in arguments.into_iter().flatten() {
            let Value::String(text) = value else {
                return Err(invalid(format!("the argument `{name}` is a string")));
            };
            let text = text.trim();
            match name.as_str() {
                "url" | "breakpoints" if text.is_empty() => {}
                "url" => args.url = Some(url(text)?),
                "breakpoints" => args.breakpoints = Some(breakpoints(text)?),
                other => {
                    return Err(invalid(format!(
                        "the prompts take url and breakpoints, not `{other}`"
                    )));
                }
            }
        }
        Ok(args)
    }
}

/// One address: at most [`MAX_URL_CHARS`] characters, no space or control character. The
/// prompt only names it; `navigate` decides what it loads.
fn url(text: &str) -> Result<String, ToolFailure> {
    let usable = text.chars().count() <= MAX_URL_CHARS
        && !text.chars().any(|c| c.is_whitespace() || c.is_control());
    if usable {
        Ok(text.to_owned())
    } else {
        Err(invalid(format!(
            "url is one address of at most {MAX_URL_CHARS} characters without spaces"
        )))
    }
}

/// Breakpoint ids separated by commas: each letters, digits, `-`, `_` or `.`, at most
/// [`MAX_BREAKPOINTS`] different ones; one named twice is kept once.
fn breakpoints(text: &str) -> Result<Vec<String>, ToolFailure> {
    let mut ids: Vec<String> = Vec::new();
    for id in text.split(',').map(str::trim) {
        let usable = !id.is_empty()
            && id.len() <= MAX_ID_CHARS
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        if !usable {
            return Err(invalid(format!(
                "breakpoints are ids separated by commas (mobile,desktop): letters, digits, \
                 - _ and ., at most {MAX_ID_CHARS} each"
            )));
        }
        if !ids.iter().any(|known| known == id) {
            ids.push(id.to_owned());
        }
    }
    if ids.len() > MAX_BREAKPOINTS {
        return Err(invalid(format!("at most {MAX_BREAKPOINTS} breakpoints")));
    }
    Ok(ids)
}

fn invalid(message: String) -> ToolFailure {
    ToolFailure::new(INVALID, message)
}

impl RbMcpServer {
    /// `prompts/list`: every prompt while `mcp.prompts` is on, else none.
    pub(crate) fn prompt_list(&self) -> Vec<Prompt> {
        if self.backend.options().prompts {
            list()
        } else {
            Vec::new()
        }
    }

    /// `prompts/get`: the prompt `name` while `mcp.prompts` is on.
    pub(crate) fn prompt_get(
        &self,
        name: &str,
        arguments: Option<&JsonObject>,
    ) -> Result<GetPromptResult, ToolFailure> {
        if !self.backend.options().prompts {
            return Err(ToolFailure::new(
                DISABLED,
                "the prompts are off: a person switches them on in Settings > Automation > MCP \
                 (rbrowser://settings/mcp.prompts)",
            )
            .with_setting(SETTING));
        }
        get(name, arguments)
    }
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
