//! Playwright locators from the recorder's selector candidates: `data-testid` as
//! `getByTestId`, role and name as `getByRole(role, { name, exact: true })`, text as
//! `getByText(text, { exact: true })`, the CSS path as `locator(css)`; `.nth(n)` for a later
//! match. Frames (`frameLocator`) and open shadow hosts (Playwright's locators pierce open shadow
//! roots) come first.
//!
//! The candidate is the first of the preferred kind, else the first one - the recorder ranks
//! them. A role Playwright's `getByRole` does not accept is skipped, so the code type-checks.

use crate::js::string;
use crate::model::{Locator, LocatorPreference, Selector};

/// The roles `page.getByRole` accepts (Playwright's `AriaRole`).
const ARIA_ROLES: &[&str] = &[
    "alert",
    "alertdialog",
    "application",
    "article",
    "banner",
    "blockquote",
    "button",
    "caption",
    "cell",
    "checkbox",
    "code",
    "columnheader",
    "combobox",
    "complementary",
    "contentinfo",
    "definition",
    "deletion",
    "dialog",
    "directory",
    "document",
    "emphasis",
    "feed",
    "figure",
    "form",
    "generic",
    "grid",
    "gridcell",
    "group",
    "heading",
    "img",
    "insertion",
    "link",
    "list",
    "listbox",
    "listitem",
    "log",
    "main",
    "marquee",
    "math",
    "meter",
    "menu",
    "menubar",
    "menuitem",
    "menuitemcheckbox",
    "menuitemradio",
    "navigation",
    "none",
    "note",
    "option",
    "paragraph",
    "presentation",
    "progressbar",
    "radio",
    "radiogroup",
    "region",
    "row",
    "rowgroup",
    "rowheader",
    "scrollbar",
    "search",
    "searchbox",
    "separator",
    "slider",
    "spinbutton",
    "status",
    "strong",
    "subscript",
    "superscript",
    "switch",
    "tab",
    "table",
    "tablist",
    "tabpanel",
    "term",
    "textbox",
    "time",
    "timer",
    "toolbar",
    "tooltip",
    "tree",
    "treegrid",
    "treeitem",
];

/// The locator of `target` on `page`, as TypeScript.
///
/// # Errors
///
/// The target has no selector Playwright can use.
pub fn locator(target: &Locator, preference: LocatorPreference) -> Result<String, String> {
    let usable: Vec<&Selector> = target.selectors.iter().filter(|s| usable(s)).collect();
    let chosen = usable
        .iter()
        .find(|selector| kind_of(selector) == preference)
        .or_else(|| usable.first())
        .ok_or_else(|| "the element has no selector".to_owned())?;
    let mut code = String::from("page");
    for frame in target.frame.iter().flatten() {
        code.push_str(&format!(".frameLocator({})", string(frame)));
    }
    for host in &target.hosts {
        code.push('.');
        code.push_str(&host_call(host));
    }
    code.push('.');
    code.push_str(&call(chosen));
    Ok(code)
}

/// Whether Playwright can use `selector` as it is.
fn usable(selector: &Selector) -> bool {
    match selector {
        Selector::Role { role, .. } => ARIA_ROLES.contains(&role.as_str()),
        Selector::Css { path } => !path.trim().is_empty(),
        Selector::Testid { testid, .. } => !testid.is_empty(),
        Selector::Text { text, .. } => !text.is_empty(),
    }
}

/// The preference `selector` answers.
fn kind_of(selector: &Selector) -> LocatorPreference {
    match selector {
        Selector::Testid { .. } => LocatorPreference::Testid,
        Selector::Role { .. } => LocatorPreference::Role,
        Selector::Text { .. } => LocatorPreference::Text,
        Selector::Css { .. } => LocatorPreference::Css,
    }
}

/// The locator method for `selector`, without the receiver.
fn call(selector: &Selector) -> String {
    let (method, nth) = match selector {
        Selector::Testid { testid, nth } => (format!("getByTestId({})", string(testid)), *nth),
        Selector::Role { role, name, nth } if name.is_empty() => {
            (format!("getByRole({})", string(role)), *nth)
        }
        Selector::Role { role, name, nth } => (
            format!(
                "getByRole({}, {{ name: {}, exact: true }})",
                string(role),
                string(name)
            ),
            *nth,
        ),
        Selector::Text { text, nth, .. } => (
            format!("getByText({}, {{ exact: true }})", string(text)),
            *nth,
        ),
        Selector::Css { path } => (format!("locator({})", string(path)), 0),
    };
    if nth > 0 {
        format!("{method}.nth({nth})")
    } else {
        method
    }
}

/// A shadow host: its one selector; a role `getByRole` does not accept as `[role="..."]`.
fn host_call(selector: &Selector) -> String {
    match selector {
        Selector::Role { role, .. } if !usable(selector) => {
            format!("locator({})", string(&format!("[role=\"{role}\"]")))
        }
        selector => call(selector),
    }
}

#[cfg(test)]
#[path = "locator_tests.rs"]
mod tests;
