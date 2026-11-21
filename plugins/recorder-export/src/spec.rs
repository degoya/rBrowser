//! The Playwright test (`<name>.spec.ts`) and its `playwright.config.ts`: one test with every
//! step, and one project per breakpoint - viewport, device scale factor, mobile, touch, user
//! agent, locale, colour scheme, reduced motion and the browser of its engine. `baseURL` is the
//! environment's origin, `RB_BASE_URL` overrides it; the steps' URLs on it are relative.

use crate::js::{SECRET_HELPER, comment, number, string};
use crate::model::{ExportRequest, Project, Recording};
use crate::steps::{Context, Guard, StepError, render};

/// The test file.
///
/// # Errors
///
/// A step cannot be written.
pub fn spec(request: &ExportRequest) -> Result<String, StepError> {
    let recording = &request.recording;
    let mut context = Context::new(
        Guard::Project,
        request.options.locator_preference,
        base_url(request),
    )
    .paced(request.options.pace)
    .least_pause(request.options.min_pause_ms)
    .sized(&recording.breakpoints);
    let body = render(&recording.steps, &mut context, 1)?;
    let mut out = format!(
        "// Exported by rBrowser (recorder-export) from the recording \"{}\".\n\
         // Run it with `npx playwright test`: playwright.config.ts makes every breakpoint a project.\n",
        comment(&recording.name)
    );
    let secrets = context.secrets.names();
    if !secrets.is_empty() {
        out.push_str(&format!(
            "// Secrets are never written here: set {} in the environment; a step stops with an \
             error when one is not set.\n",
            secrets.join(", ")
        ));
    }
    out.push_str("import { test, expect } from '@playwright/test';\n\n");
    if !secrets.is_empty() {
        out.push_str(SECRET_HELPER);
        out.push('\n');
    }
    out.push_str(&format!(
        "test({}, async ({{ page }}) => {{\n",
        string(&recording.name)
    ));
    out.push_str(&body);
    out.push_str("});\n");
    Ok(out)
}

/// The config with one project per breakpoint.
pub fn config(request: &ExportRequest) -> String {
    let recording = &request.recording;
    let mut out = format!(
        "// Exported by rBrowser (recorder-export) from the recording \"{}\": one project per\n\
         // breakpoint. RB_BASE_URL runs the test against another origin.\n\
         import {{ defineConfig }} from '@playwright/test';\n\n\
         export default defineConfig({{\n  testDir: '.',\n  fullyParallel: true,\n  use: {{\n",
        comment(&recording.name)
    );
    match base_url(request) {
        Some(base) => out.push_str(&format!(
            "    baseURL: process.env.RB_BASE_URL ?? {},\n",
            string(&base)
        )),
        None => out.push_str("    baseURL: process.env.RB_BASE_URL,\n"),
    }
    out.push_str("  },\n");
    let projects = projects(request);
    if !projects.is_empty() {
        out.push_str("  projects: [\n");
        for project in &projects {
            out.push_str(&project_entry(project));
        }
        out.push_str("  ],\n");
    }
    out.push_str("});\n");
    out
}

/// The base URL: the host's (the environment's origin), else the start URL's origin - lower
/// case, without user info and default port (`rb_util::url::origin_of`, RB-094-10).
pub fn base_url(request: &ExportRequest) -> Option<String> {
    request
        .options
        .base_url
        .clone()
        .filter(|base| !base.is_empty())
        .or_else(|| {
            request
                .recording
                .start_url
                .as_deref()
                .and_then(rb_util::url::origin_of)
        })
}

/// The projects: the host's, else the recording's breakpoints; unnamed and repeated names
/// dropped.
pub fn projects(request: &ExportRequest) -> Vec<Project> {
    let given = if request.options.projects.is_empty() {
        from_recording(&request.recording)
    } else {
        request.options.projects.clone()
    };
    let mut seen: Vec<String> = Vec::new();
    given
        .into_iter()
        .filter(|project| {
            let fresh = !project.name.is_empty() && !seen.contains(&project.name);
            if fresh {
                seen.push(project.name.clone());
            }
            fresh
        })
        .collect()
}

fn from_recording(recording: &Recording) -> Vec<Project> {
    recording
        .breakpoints
        .iter()
        .map(|breakpoint| Project {
            name: breakpoint.id.clone(),
            width: breakpoint.width,
            height: breakpoint.height,
            device_scale_factor: Some(breakpoint.device_scale_factor),
            ..Project::default()
        })
        .collect()
}

/// One entry of `projects`.
fn project_entry(project: &Project) -> String {
    let browser = match project.engine.as_deref() {
        Some("firefox") => "firefox",
        Some("webkit") => "webkit",
        _ => "chromium",
    };
    let mut fields = vec![
        format!("browserName: {}", string(browser)),
        format!(
            "viewport: {{ width: {}, height: {} }}",
            project.width, project.height
        ),
    ];
    if let Some(scale) = project
        .device_scale_factor
        .filter(|scale| scale.is_finite() && *scale > 0.0)
    {
        fields.push(format!("deviceScaleFactor: {}", number(scale)));
    }
    // Playwright refuses `isMobile` for Firefox.
    if let Some(mobile) = project.is_mobile.filter(|_| browser != "firefox") {
        fields.push(format!("isMobile: {mobile}"));
    }
    if let Some(touch) = project.has_touch {
        fields.push(format!("hasTouch: {touch}"));
    }
    if let Some(agent) = project.user_agent.as_deref().filter(|ua| !ua.is_empty()) {
        fields.push(format!("userAgent: {}", string(agent)));
    }
    if let Some(locale) = project.locale.as_deref().filter(|l| !l.is_empty()) {
        fields.push(format!("locale: {}", string(locale)));
    }
    if let Some(scheme @ ("light" | "dark" | "no-preference")) = project.color_scheme.as_deref() {
        fields.push(format!("colorScheme: {}", string(scheme)));
    }
    if let Some(reduce) = project.reduced_motion {
        let motion = if reduce { "reduce" } else { "no-preference" };
        fields.push(format!("reducedMotion: {}", string(motion)));
    }
    let mut out = format!(
        "    {{\n      name: {},\n      use: {{\n",
        string(&project.name)
    );
    for field in fields {
        out.push_str(&format!("        {field},\n"));
    }
    out.push_str("      },\n    },\n");
    out
}

#[cfg(test)]
#[path = "spec_tests.rs"]
mod tests;
