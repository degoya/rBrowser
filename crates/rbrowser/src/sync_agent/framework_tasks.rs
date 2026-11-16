//! The tasks that keep [`super::FrameworkHooks`] current (RB-080-24): the user settings' modes
//! at start and on every change, the agents' framework reports (a found framework, in `auto`,
//! gets its hook for the view's next document) and `ready` of a new top document (the old
//! report goes), a closed view; after each the shell gets the views' frameworks for its offer.

use std::sync::Arc;

use rb_engine::{ViewEvent, ViewHost};
use rb_sync::AgentMessage;
use tokio::sync::broadcast::error::RecvError;

use crate::control::Core;
use crate::host::Host;

/// Applies the user settings and starts the listeners of `core`'s window.
pub fn on_start(core: &Core) {
    let host = core.host().clone();
    let modes = core.user_settings().framework_devtools;
    core.runtime().spawn(async move {
        host.framework_hooks().set_modes(modes).await;
        publish(&host);
    });
    watch_views(core.host());
    watch_reports(core.host());
}

/// The user settings changed: the modes follow, scripts are registered or removed at once.
pub fn on_user_settings_changed(core: &Core) {
    let host = core.host().clone();
    let modes = core.user_settings().framework_devtools;
    core.runtime().spawn(async move {
        host.framework_hooks().set_modes(modes).await;
        publish(&host);
    });
}

/// Hands the shell the views' frameworks.
fn publish(host: &Host) {
    let frameworks = host.framework_hooks().frameworks();
    host.update_shell_state(|shell| shell.frameworks = frameworks);
}

fn watch_views(host: &Arc<Host>) {
    let mut events = host.subscribe();
    let weak = Arc::downgrade(host);
    host.runtime().spawn(async move {
        loop {
            let event = match events.recv().await {
                Ok(event) => event,
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return,
            };
            let Some(host) = weak.upgrade() else {
                return;
            };
            if let ViewEvent::Closed(view) = event {
                host.framework_hooks().forget(view);
                publish(&host);
            }
        }
    });
}

fn watch_reports(host: &Arc<Host>) {
    let mut reports = host.sync_agents().subscribe();
    let weak = Arc::downgrade(host);
    host.runtime().spawn(async move {
        loop {
            let report = match reports.recv().await {
                Ok(report) => report,
                Err(RecvError::Lagged(missed)) => {
                    tracing::warn!(missed, "agent reports dropped (framework devtools)");
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            let Some(host) = weak.upgrade() else {
                return;
            };
            match &report.message {
                AgentMessage::Framework(message) if message.top => {
                    host.framework_hooks().on_report(report.view, message).await;
                    publish(&host);
                }
                AgentMessage::Ready(ready) if ready.top => {
                    host.framework_hooks().on_navigated(report.view);
                    publish(&host);
                }
                _ => {}
            }
        }
    });
}
