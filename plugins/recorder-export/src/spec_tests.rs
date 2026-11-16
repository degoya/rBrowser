use super::{base_url, config, projects, spec};
use crate::model::{ExportOptions, ExportRequest, Project, parse_request};

fn request(steps: &str, options: &str) -> ExportRequest {
    parse_request(&format!(
        r#"{{ "recording": {{ "version": 1, "name": "login",
            "startUrl": "http://127.0.0.1:8000/recorder.html",
            "breakpoints": [
              {{ "id": "mobile", "width": 390, "height": 844, "deviceScaleFactor": 3 }},
              {{ "id": "desktop", "width": 1440, "height": 900, "deviceScaleFactor": 1 }}
            ],
            "startedAtMs": 1, "appVersion": "0.5.0", "steps": [{steps}] }},
          "options": {options} }}"#
    ))
    .expect("a request")
}

#[test]
fn every_breakpoint_of_the_recording_is_a_project() {
    let request = request("", "{}");
    let names: Vec<String> = projects(&request).into_iter().map(|p| p.name).collect();
    assert_eq!(names, ["mobile", "desktop"]);
    let text = config(&request);
    assert_eq!(text.matches("      name: ").count(), 2, "{text}");
    assert!(text.contains(
        "      name: 'mobile',\n      use: {\n        browserName: 'chromium',\n        \
         viewport: { width: 390, height: 844 },\n        deviceScaleFactor: 3,\n      },\n"
    ));
}

#[test]
fn the_hosts_projects_carry_the_whole_emulation() {
    let request = request(
        "",
        r#"{ "projects": [
            { "name": "phone", "width": 390, "height": 844, "deviceScaleFactor": 3,
              "isMobile": true, "hasTouch": true, "userAgent": "Agent 'x'", "locale": "de-DE",
              "colorScheme": "dark", "reducedMotion": true },
            { "name": "fox", "engine": "firefox", "width": 800, "height": 600, "isMobile": true },
            { "name": "safari", "engine": "webkit", "width": 1024, "height": 768,
              "colorScheme": "sepia", "reducedMotion": false }
        ] }"#,
    );
    let text = config(&request);
    for field in [
        "browserName: 'chromium'",
        "viewport: { width: 390, height: 844 }",
        "deviceScaleFactor: 3",
        "isMobile: true",
        "hasTouch: true",
        r"userAgent: 'Agent \'x\''",
        "locale: 'de-DE'",
        "colorScheme: 'dark'",
        "reducedMotion: 'reduce'",
        "browserName: 'firefox'",
        "browserName: 'webkit'",
        "reducedMotion: 'no-preference'",
    ] {
        assert!(text.contains(field), "{field} missing in\n{text}");
    }
    assert_eq!(text.matches("isMobile").count(), 1, "not for Firefox");
    assert!(!text.contains("sepia"));
}

#[test]
fn a_repeated_or_empty_project_name_is_dropped() {
    let request = ExportRequest {
        options: ExportOptions {
            projects: vec![
                Project {
                    name: "a".to_owned(),
                    ..Project::default()
                },
                Project::default(),
                Project {
                    name: "a".to_owned(),
                    ..Project::default()
                },
            ],
            ..ExportOptions::default()
        },
        ..request("", "{}")
    };
    assert_eq!(projects(&request).len(), 1);
}

#[test]
fn the_base_url_is_the_hosts_else_the_start_urls_origin() {
    assert_eq!(
        base_url(&request("", "{}")).as_deref(),
        Some("http://127.0.0.1:8000")
    );
    assert_eq!(
        base_url(&request(
            "",
            r#"{ "baseUrl": "https://staging.example.org" }"#
        ))
        .as_deref(),
        Some("https://staging.example.org")
    );
    let text = config(&request("", "{}"));
    assert!(text.contains("    baseURL: process.env.RB_BASE_URL ?? 'http://127.0.0.1:8000',\n"));
}

#[test]
fn the_spec_is_one_test_named_after_the_recording() {
    let text = spec(&request(
        r#"{ "id": 1, "action": { "type": "navigate", "url": "http://127.0.0.1:8000/recorder.html" },
             "view": "mobile", "atMs": 0 }"#,
        "{}",
    ))
    .expect("a spec");
    assert_eq!(
        text,
        "// Exported by rBrowser (recorder-export) from the recording \"login\".\n\
         // Run it with `npx playwright test`: playwright.config.ts makes every breakpoint a project.\n\
         import { test, expect } from '@playwright/test';\n\n\
         test('login', async ({ page }) => {\n  \
         // Step 1: navigate\n  await page.goto('/recorder.html');\n});\n"
    );
}

#[test]
fn the_spec_names_the_secrets_it_needs_but_never_their_values() {
    let text = spec(&request(
        r##"{ "id": 2, "action": { "type": "fill", "value": "{{secret:password}}", "secret": true,
               "target": { "selectors": [{ "kind": "css", "path": "#password" }] } },
             "view": "mobile", "atMs": 0 }"##,
        "{}",
    ))
    .expect("a spec");
    assert!(
        text.contains("set RB_SECRET_PASSWORD in the environment"),
        "{text}"
    );
    assert!(
        text.contains(".fill(rbSecret('RB_SECRET_PASSWORD'));"),
        "{text}"
    );
    // The helper sits after the import and throws when the variable is missing.
    let import = text.find("import { test").expect("the import");
    let helper = text.find("function rbSecret(").expect("the helper");
    assert!(import < helper, "{text}");
    assert!(text.contains("throw new Error("), "{text}");
    assert!(!text.contains("{{secret"), "{text}");
}
