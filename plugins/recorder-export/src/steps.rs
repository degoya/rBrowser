//! The steps of a recording as Playwright statements, shared by the spec and the flow: both
//! run against `page` with `expect` from `@playwright/test`.
//!
//! Every step starts with a stable comment (`// Step <id>: <kind>[ - <name>]`) - its id stays
//! when steps are reordered, so a diff between two exports shows what changed. An `onlyIn` step
//! runs inside a guard: in the spec on the project's name (`test.info().project.name`), in the
//! flow on the view's breakpoint (`view.breakpoint`).

use crate::js::{Secrets, comment, number, relative_url, secret as secret_value, string};
use crate::locator::locator;
use crate::model::{Action, Assertion, Locator, LocatorPreference, Step};

/// Where the steps go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Guard {
    /// A Playwright test: `onlyIn` compares the project's name.
    Project,
    /// An rBrowser flow: `onlyIn` compares the view's breakpoint.
    Breakpoint,
}

/// What every step of one file shares, and the secrets it used.
#[derive(Clone, Debug)]
pub struct Context {
    guard: Guard,
    preference: LocatorPreference,
    base_url: Option<String>,
    /// The environment variables of the secrets used so far.
    pub secrets: Secrets,
}

impl Context {
    /// A context for `guard`; URLs on `base_url`'s origin are written relative to it.
    pub fn new(guard: Guard, preference: LocatorPreference, base_url: Option<String>) -> Self {
        Self {
            guard,
            preference,
            base_url,
            secrets: Secrets::default(),
        }
    }
}

/// A step the exporter cannot write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepError {
    /// The step's id.
    pub step: u32,
    /// Why.
    pub reason: String,
}

/// `steps` as statements, each line indented by `depth` levels of two spaces.
///
/// # Errors
///
/// A step has no selector Playwright can use.
pub fn render(steps: &[Step], context: &mut Context, depth: usize) -> Result<String, StepError> {
    let mut out = String::new();
    for step in steps {
        let statement = statement(&step.action, context).map_err(|reason| StepError {
            step: step.id,
            reason,
        })?;
        let mut title = format!("Step {}: {}", step.id, kind(&step.action));
        if let Some(name) = step.name.as_deref().map(comment).filter(|n| !n.is_empty()) {
            title.push_str(&format!(" - {name}"));
        }
        let only_in: Vec<&str> = step
            .only_in
            .iter()
            .map(String::as_str)
            .filter(|id| !id.is_empty())
            .collect();
        if !only_in.is_empty() {
            title.push_str(&format!(" (only in {})", comment(&only_in.join(", "))));
        }
        line(&mut out, depth, &format!("// {title}"));
        if only_in.is_empty() {
            line(&mut out, depth, &statement);
            continue;
        }
        let list = only_in
            .iter()
            .map(|id| string(id))
            .collect::<Vec<_>>()
            .join(", ");
        let subject = match context.guard {
            Guard::Project => "test.info().project.name",
            Guard::Breakpoint => "view.breakpoint",
        };
        line(
            &mut out,
            depth,
            &format!("if ([{list}].includes({subject})) {{"),
        );
        line(&mut out, depth + 1, &statement);
        line(&mut out, depth, "}");
    }
    Ok(out)
}

fn line(out: &mut String, depth: usize, text: &str) {
    out.push_str(&"  ".repeat(depth));
    out.push_str(text);
    out.push('\n');
}

/// The step's kind in its comment.
fn kind(action: &Action) -> &'static str {
    match action {
        Action::Navigate { .. } => "navigate",
        Action::Click { .. } => "click",
        Action::Dblclick { .. } => "dblclick",
        Action::Fill { .. } => "fill",
        Action::Press { .. } => "press",
        Action::Select { .. } => "select",
        Action::Check { .. } => "check",
        Action::Uncheck { .. } => "uncheck",
        Action::Hover { .. } => "hover",
        Action::Scroll { .. } => "scroll",
        Action::WaitForUrl { .. } => "waitForUrl",
        Action::Assert { assertion } => match assertion {
            Assertion::Visible { .. } => "assert visible",
            Assertion::Text { .. } => "assert text",
            Assertion::Value { .. } => "assert value",
            Assertion::Url { .. } => "assert url",
        },
    }
}

/// The one statement of `action`.
fn statement(action: &Action, context: &mut Context) -> Result<String, String> {
    let preference = context.preference;
    let at = |target: &Locator| locator(target, preference);
    Ok(match action {
        Action::Navigate { url } => format!("await page.goto({});", url_expr(url, context)),
        Action::Click {
            target,
            button,
            modifiers,
        } => {
            let mut options = Vec::new();
            if let Some(button @ ("right" | "middle")) = button.as_deref() {
                options.push(format!("button: {}", string(button)));
            }
            let keys = modifier_names(*modifiers);
            if !keys.is_empty() {
                let keys: Vec<String> = keys.iter().map(|key| string(key)).collect();
                options.push(format!("modifiers: [{}]", keys.join(", ")));
            }
            if options.is_empty() {
                format!("await {}.click();", at(target)?)
            } else {
                format!("await {}.click({{ {} }});", at(target)?, options.join(", "))
            }
        }
        Action::Dblclick { target } => format!("await {}.dblclick();", at(target)?),
        Action::Fill {
            target,
            value: text,
            secret,
        } => {
            let target = at(target)?;
            let value = if *secret {
                secret_value(text, &mut context.secrets).ok_or_else(|| {
                    "a secret fill holds no `{{secret:<name>}}` placeholder".to_owned()
                })?
            } else {
                string(text)
            };
            format!("await {target}.fill({value});")
        }
        Action::Press {
            target,
            key,
            modifiers,
        } => {
            let mut chord: Vec<&str> = modifier_names(*modifiers);
            chord.push(key);
            let chord = string(&chord.join("+"));
            match target {
                Some(target) => format!("await {}.press({chord});", at(target)?),
                None => format!("await page.keyboard.press({chord});"),
            }
        }
        Action::Select {
            target,
            value: chosen,
            indexes,
        } => {
            let target = at(target)?;
            let argument = match (chosen, indexes.as_slice()) {
                (_, several) if several.len() > 1 => {
                    let list: Vec<String> = several
                        .iter()
                        .map(|index| format!("{{ index: {index} }}"))
                        .collect();
                    format!("[{}]", list.join(", "))
                }
                (Some(chosen), _) => string(chosen),
                (None, [index]) => format!("{{ index: {index} }}"),
                (None, _) => "[]".to_owned(),
            };
            format!("await {target}.selectOption({argument});")
        }
        Action::Check { target } => format!("await {}.check();", at(target)?),
        Action::Uncheck { target } => format!("await {}.uncheck();", at(target)?),
        Action::Hover { target } => format!("await {}.hover();", at(target)?),
        Action::Scroll { target, x, y } => {
            let (x, y) = (number(*x), number(*y));
            match target {
                Some(target) => format!(
                    "await {}.evaluate((element) => element.scrollTo({x}, {y}));",
                    at(target)?
                ),
                None => format!("await page.evaluate(() => window.scrollTo({x}, {y}));"),
            }
        }
        Action::WaitForUrl { url } => {
            format!("await expect(page).toHaveURL({});", url_expr(url, context))
        }
        Action::Assert { assertion } => match assertion {
            Assertion::Visible { target } => {
                format!("await expect({}).toBeVisible();", at(target)?)
            }
            Assertion::Text { target, text } => {
                let target = at(target)?;
                format!("await expect({target}).toHaveText({});", string(text))
            }
            Assertion::Value {
                target,
                value: expected,
            } => {
                let target = at(target)?;
                format!("await expect({target}).toHaveValue({});", string(expected))
            }
            Assertion::Url { url } => {
                format!("await expect(page).toHaveURL({});", url_expr(url, context))
            }
        },
    })
}

/// A URL as a literal: relative to the base URL in the spec. Never a secret.
fn url_expr(url: &str, context: &Context) -> String {
    string(&relative_url(url, context.base_url.as_deref()))
}

/// The CDP modifier bits (1 Alt, 2 Ctrl, 4 Meta, 8 Shift) as Playwright's names.
fn modifier_names(bits: u8) -> Vec<&'static str> {
    [(1, "Alt"), (2, "Control"), (4, "Meta"), (8, "Shift")]
        .into_iter()
        .filter(|(bit, _)| bits & bit != 0)
        .map(|(_, name)| name)
        .collect()
}

#[cfg(test)]
#[path = "steps_tests.rs"]
mod tests;
