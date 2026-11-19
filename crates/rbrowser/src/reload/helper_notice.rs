//! What `wsl.restart_helper` shows in the status bar (RB-090-85): its start at once, and its end
//! once the watch settled - the helper again, the polling that took its place, or the failure.
//! No control shows the watch state, so without them the palette and the shortcut said nothing.

use std::collections::BTreeMap;
use std::time::Duration;

use rb_ipc::{CoreToShell, Notification, NotificationLevel};

use super::service::HotReload;
use super::status::{WatcherInfo, WatcherStatus};
use crate::control::Core;

/// The notice that the helper restarts.
pub const RESTARTING_KEY: &str = "wsl.notice.restarting";
/// The notice that the watch settled after the restart; `{state}`.
pub const RESTARTED_KEY: &str = "wsl.notice.restarted";
/// The settings section of the WSL watch helper.
const SETTINGS_KEY: &str = "wsl-watcher";
/// How long the end is waited for; a restart that never settles is only logged.
const SETTLE_WAIT: Duration = Duration::from_secs(60);

/// Names the restart of `service`'s helper, restarts it with `restart` and names the end once
/// the watch settled. `restart` answers whether the project is watched through the helper.
pub fn follow(core: &Core, service: &HotReload, restart: impl FnOnce() -> bool) -> bool {
    let mut info = service.subscribe();
    info.mark_unchanged();
    if !restart() {
        return false;
    }
    core.notify(restarting_notice());
    let Some(core) = core.arc() else {
        return true;
    };
    let runtime = core.runtime().clone();
    runtime.spawn(async move {
        let settled = tokio::time::timeout(SETTLE_WAIT, async {
            loop {
                if info.changed().await.is_err() {
                    return None;
                }
                let now = info.borrow_and_update().clone();
                if is_settled(&now.status) {
                    return Some(now);
                }
            }
        })
        .await;
        match settled {
            Ok(Some(now)) => core.notify(restarted_notice(&now)),
            _ => tracing::debug!("the WSL helper's watch did not settle after its restart"),
        }
    });
    true
}

/// The notice that the helper restarts.
pub fn restarting_notice() -> CoreToShell {
    notice(NotificationLevel::Info, RESTARTING_KEY, BTreeMap::new())
}

/// The notice of the watch the restart settled on: success for the helper, a warning for
/// another watch, an error for none.
pub fn restarted_notice(info: &WatcherInfo) -> CoreToShell {
    let level = match info.status {
        WatcherStatus::Helper { .. } => NotificationLevel::Success,
        WatcherStatus::Failed { .. } => NotificationLevel::Error,
        _ => NotificationLevel::Warning,
    };
    let args = BTreeMap::from([("state".to_owned(), rb_i18n::t(info.status.label_key()))]);
    notice(level, RESTARTED_KEY, args)
}

/// Whether the watch is past its start.
pub fn is_settled(status: &WatcherStatus) -> bool {
    !matches!(status, WatcherStatus::Starting)
}

fn notice(level: NotificationLevel, key: &str, args: BTreeMap<String, String>) -> CoreToShell {
    CoreToShell::Notification(Notification {
        level,
        message_key: key.to_owned(),
        args,
        settings_key: Some(SETTINGS_KEY.to_owned()),
    })
}

#[cfg(test)]
#[path = "helper_notice_tests.rs"]
mod tests;
