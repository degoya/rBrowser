use super::flow;
use crate::model::parse_request;

const RECORDING: &str = r##"{ "version": 1, "name": "login",
    "startUrl": "http://127.0.0.1:8000/recorder.html",
    "breakpoints": [{ "id": "mobile", "width": 390, "height": 844, "deviceScaleFactor": 3 }],
    "startedAtMs": 1, "appVersion": "0.5.0",
    "steps": [
      { "id": 1, "action": { "type": "navigate", "url": "http://127.0.0.1:8000/recorder.html" },
        "view": "mobile", "atMs": 0 },
      { "id": 2, "action": { "type": "fill", "value": "{{secret:password}}", "secret": true,
          "target": { "selectors": [{ "kind": "css", "path": "#password" }] } },
        "view": "mobile", "atMs": 1 },
      { "id": 3, "action": { "type": "click",
          "target": { "selectors": [{ "kind": "role", "role": "button", "name": "Menu" },
                                    { "kind": "css", "path": "#burger" }] } },
        "view": "mobile", "atMs": 2, "onlyIn": ["mobile"] }
    ] }"##;

#[test]
fn the_flow_has_no_imports_and_uses_the_injected_globals() {
    let text = flow(&parse_request(RECORDING).expect("a recording")).expect("a flow");
    assert_eq!(
        text,
        "// @description Exported by rBrowser (recorder-export) from the recording \"login\".\n\
         // Secrets are never written here: the steps read RB_SECRET_PASSWORD from the \
         environment and stop with an error when one is not set. rBrowser's flow runner passes \
         no environment variables to a flow yet.\n\
         function rbSecret(name: string): string {\n\
         \x20 const value = process.env[name];\n\
         \x20 if (value === undefined || value === '') {\n\
         \x20   throw new Error(`The secret ${name} is not set in the environment.`);\n\
         \x20 }\n\
         \x20 return value;\n\
         }\n\
         // Step 1: navigate\n\
         await step('Step 1: navigate', async () => {\n  \
         await page.goto('http://127.0.0.1:8000/recorder.html');\n\
         });\n\
         // Step 2: fill\n\
         await step('Step 2: fill', async () => {\n  \
         await page.locator('#password').fill(rbSecret('RB_SECRET_PASSWORD'));\n\
         });\n\
         // Step 3: click (only in mobile)\n\
         await step('Step 3: click', async () => {\n  \
         if (['mobile'].includes(view.breakpoint)) {\n    \
         await page.getByRole('button', { name: 'Menu', exact: true }).filter({ visible: true }).first().click();\n  }\n\
         });\n"
    );
    assert!(!text.contains("import "));
}
