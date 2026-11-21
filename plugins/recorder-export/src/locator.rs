//! Playwright locators from the recorder's selector candidates: `data-testid` as
//! `getByTestId`, role and name as `getByRole(role, { name, exact: true })`, text as the
//! elements of its recorded tag whose whole text it is (`locator(tag).filter({ hasText:
//! /^\s*text\s*$/ })`, RB-090-93), the CSS path as `locator(css)`; `.nth(n)` for a later test id
//! match. Frames (`frameLocator`) and open shadow hosts (Playwright's locators pierce
//! open shadow roots) come first.
//!
//! A role or text locator of the element counts the matches a view shows (RB-090-81,
//! Playwright 1.51 and later): `.filter({ visible: true })` and then `.first()`, or
//! `.nth(visibleNth)` for the later visible match the person picked (the fourth of several
//! cards' "Read more"). The leader's document position is not every view's - a breakpoint hides
//! the desktop navigation, another its mobile copy - and a name two elements share is no
//! strict-mode error. A later match recorded without its visible index (a recording made
//! before, a hidden element) keeps `.nth(n)`. The CSS path names one element and stays as it is.
//! A role or text the page showed more than once comes with the ancestor that makes it unique
//! (`within`, RB-090-84): `locator(within).getByRole(...)`, so a view that shows the other match
//! first - a content link while the menu is still closed - waits for the one the person picked.
//!
//! The candidate is the first of the preferred kind, else the first one - the recorder ranks
//! them. A role Playwright's `getByRole` does not accept is skipped, so the code type-checks.

use crate::js::{string, whole_text};
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
    code.push_str(&element_call(chosen));
    Ok(code)
}

/// The element's own locator method: a role or text match among the visible ones (RB-090-81),
/// inside the ancestor that makes it unique when the recording names one (RB-090-84).
fn element_call(selector: &Selector) -> String {
    let (Selector::Role {
        nth,
        visible_nth,
        within,
        ..
    }
    | Selector::Text {
        nth,
        visible_nth,
        within,
        ..
    }) = selector
    else {
        return call(selector);
    };
    let method = match within.as_deref().filter(|css| !css.trim().is_empty()) {
        Some(css) => format!("locator({}).{}", string(css), method(selector).0),
        None => method(selector).0,
    };
    match (*visible_nth, *nth) {
        (Some(0), _) | (None, 0) => {
            format!("{method}.filter({{ visible: true }}).first()")
        }
        (Some(index), _) => format!("{method}.filter({{ visible: true }}).nth({index})"),
        (None, nth) => format!("{method}.nth({nth})"),
    }
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

/// The locator method for `selector`, without the receiver, with `.nth(n)` for a later match.
fn call(selector: &Selector) -> String {
    match method(selector) {
        (method, 0) => method,
        (method, nth) => format!("{method}.nth({nth})"),
    }
}

/// The locator method for `selector` and the index of its match.
fn method(selector: &Selector) -> (String, u32) {
    match selector {
        Selector::Testid { testid, nth } => (format!("getByTestId({})", string(testid)), *nth),
        Selector::Role {
            role, name, nth, ..
        } if name.is_empty() => (format!("getByRole({})", string(role)), *nth),
        Selector::Role {
            role, name, nth, ..
        } => (
            format!(
                "getByRole({}, {{ name: {}, exact: true }})",
                string(role),
                string(name)
            ),
            *nth,
        ),
        Selector::Text { tag, text, nth, .. } => (text_call(tag, text), *nth),
        Selector::Css { path } => (format!("locator({})", string(path)), 0),
    }
}

/// The elements of the recorded `tag` whose whole text is `text` (RB-090-93) - the agent's own
/// matches (`matchAll` in `agent/src/selector.ts`), so `nth` and `visibleNth` count the same
/// elements. `getByText` finds the innermost element with the text - a visually hidden span in
/// a burger label, which the label covers - and climbing from it to the tag would also take a
/// `p` that only contains a matching `span`, or the inner one of two nested `div`s. A tag that is
/// no plain lower-case name stays out of the code: the text alone, `getByText` as before.
fn text_call(tag: &str, text: &str) -> String {
    let mut chars = tag.chars();
    let plain = chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if plain {
        format!(
            "locator({}).filter({{ hasText: {} }})",
            string(tag),
            whole_text(text)
        )
    } else {
        format!("getByText({}, {{ exact: true }})", string(text))
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
