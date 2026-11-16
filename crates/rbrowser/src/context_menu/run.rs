//! Runs a chosen entry: the registry's command on the core's runtime, never on the thread the
//! menu was chosen on (CEF's UI thread); a failure becomes a notification in the status bar.

use std::sync::Arc;

use rb_engine::ViewId;

use super::model::Action;
use crate::control::Core;
use crate::form_sync::skip_next_click_in;

/// Runs `action` for `view` off the calling thread.
pub fn run(core: &Arc<Core>, view: ViewId, action: &Action) {
    match action {
        Action::Run { command, args } => {
            let (notifier, id) = (core.clone(), command.clone());
            core.run_command_async(command, args.clone(), move |result| {
                if let Err(error) = result {
                    tracing::warn!(command = %id, %view, %error, "the context menu's command failed");
                    let title = notifier.command_title(&id);
                    notifier.notify(crate::control::failure_notification(&title, &error));
                }
            });
        }
        Action::SkipNextClick => {
            let runtime = core.runtime().clone();
            let core = core.clone();
            runtime.spawn_blocking(move || {
                if let Err(error) = skip_next_click_in(&core, view) {
                    tracing::warn!(%view, %error, "the next click could not be skipped");
                }
            });
        }
    }
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
