//! RB-090-78: a recording whose form the autofill filled - the steps the recorder writes for it
//! (`rb_sync::recorder::autofill`): a fill per text field with the value set, a password as its
//! placeholder, a check, a select, and a field the views set to their own values (a German
//! phone number in the German view). Exported as a flow, it fills the same values.

use super::{FLOW, render};

fn role(role: &str, name: &str, path: &str) -> serde_json::Value {
    serde_json::json!({ "selectors": [
        { "kind": "role", "role": role, "name": name }, { "kind": "css", "path": path } ] })
}

fn recording() -> String {
    let steps = serde_json::json!([
        { "id": 1, "action": { "type": "navigate", "url": "http://127.0.0.1:8000/forms/register.html" },
          "view": "desktop", "atMs": 0 },
        { "id": 2, "action": { "type": "fill", "target": role("textbox", "Email", "#email"),
                               "value": "ada@example.com" }, "view": "desktop", "atMs": 900 },
        { "id": 3, "action": { "type": "fill",
                               "target": { "selectors": [ { "kind": "css", "path": "#password" } ] },
                               "value": "{{secret:password}}", "secret": true },
          "view": "desktop", "atMs": 900 },
        { "id": 4, "action": { "type": "check", "target": role("checkbox", "I agree", "#terms") },
          "view": "desktop", "atMs": 900 },
        { "id": 5, "action": { "type": "select", "target": role("combobox", "Country", "#country"),
                               "value": "de", "indexes": [1] }, "view": "desktop", "atMs": 900 },
        { "id": 6, "action": { "type": "fill", "target": role("textbox", "Phone", "#phone"),
                               "value": "+1 202 555 0143" }, "view": "desktop", "atMs": 900,
          "onlyIn": ["desktop"] },
        { "id": 7, "action": { "type": "fill", "target": role("textbox", "Phone", "#phone"),
                               "value": "+49 30 1234567" }, "view": "mobile", "atMs": 900,
          "onlyIn": ["mobile"] }
    ]);
    serde_json::json!({
        "version": 1, "name": "register", "startUrl": "http://127.0.0.1:8000/forms/register.html",
        "breakpoints": [
            { "id": "desktop", "width": 1440, "height": 900, "deviceScaleFactor": 1 },
            { "id": "mobile", "width": 390, "height": 844, "deviceScaleFactor": 3 } ],
        "startedAtMs": 1_794_330_000_000_u64, "appVersion": "0.9.0", "steps": steps
    })
    .to_string()
}

#[test]
fn the_flow_fills_the_values_the_autofill_set() {
    let flow = render(FLOW, &recording()).expect("rendered").text;
    for line in [
        "getByRole('textbox', { name: 'Email', exact: true }).fill('ada@example.com');",
        "getByRole('checkbox', { name: 'I agree', exact: true }).check();",
        "getByRole('combobox', { name: 'Country', exact: true }).selectOption('de');",
        "if (['desktop'].includes(view.breakpoint)) {",
        "if (['mobile'].includes(view.breakpoint)) {",
        ".fill('+1 202 555 0143');",
        ".fill('+49 30 1234567');",
    ] {
        assert!(flow.contains(line), "{line}\n{flow}");
    }
}

#[test]
fn the_password_comes_from_the_environment() {
    let flow = render(FLOW, &recording()).expect("rendered").text;
    assert!(
        flow.contains(".fill(rbSecret('RB_SECRET_PASSWORD'));"),
        "{flow}"
    );
    assert!(!flow.contains("{{secret:"), "{flow}");
}
