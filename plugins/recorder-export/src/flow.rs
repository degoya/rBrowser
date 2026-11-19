//! The rBrowser flow (`.rbrowser/flows/<name>.ts`, RB-040-07): the steps as top-level
//! statements against the injected `page`, `view` and `expect` - no imports. A flow runs in the
//! view's own page, which has no base URL, so its URLs stay absolute; an `onlyIn` step compares
//! `view.breakpoint`.

use crate::js::{SECRET_HELPER, comment};
use crate::model::ExportRequest;
use crate::steps::{Context, Guard, StepError, render};

/// The flow file.
///
/// # Errors
///
/// A step cannot be written.
pub fn flow(request: &ExportRequest) -> Result<String, StepError> {
    let recording = &request.recording;
    let mut context = Context::new(Guard::Breakpoint, request.options.locator_preference, None)
        .paced(request.options.pace)
        .least_pause(request.options.min_pause_ms);
    let body = render(&recording.steps, &mut context, 0)?;
    let mut out = format!(
        "// @description Exported by rBrowser (recorder-export) from the recording \"{}\".\n",
        comment(&recording.name)
    );
    let secrets = context.secrets.names();
    if !secrets.is_empty() {
        // The flow worker gets no environment variables (`sidecar/src/modules/flows/spawn.ts`),
        // so a flow with secrets stops at its first secret step - loudly, with the name.
        out.push_str(&format!(
            "// Secrets are never written here: the steps read {} from the environment and stop \
             with an error when one is not set. rBrowser's flow runner passes no environment \
             variables to a flow yet.\n",
            secrets.join(", ")
        ));
        out.push_str(SECRET_HELPER);
    }
    out.push_str(&body);
    Ok(out)
}

#[cfg(test)]
#[path = "flow_tests.rs"]
mod tests;
