//! Follows the project config on disk: a change of `.rbrowser/config.json` or
//! `config.local.json` reloads it without a restart (RB-010-04). The watcher of the project's
//! files (hot reload, `rbrowser-wsl watch` inside WSL) is RB-030-04; this one only looks at the
//! config folder.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use rb_config::{CONFIG_DIR, CONFIG_FILE, LOCAL_CONFIG_FILE};

/// Quiet time after the last change before the config is read (editors write in steps).
pub const DEBOUNCE: Duration = Duration::from_millis(300);

/// Keeps the watch alive; dropping it stops the watch.
pub struct ConfigWatch {
    _watcher: notify::RecommendedWatcher,
}

/// Calls `on_change` (on a thread of its own) after the config files of the project at `root`
/// changed and stayed quiet for [`DEBOUNCE`].
pub fn watch_config(
    root: &Path,
    on_change: impl Fn() + Send + 'static,
) -> notify::Result<ConfigWatch> {
    let folder = root.join(CONFIG_DIR);
    let (sender, receiver) = mpsc::channel::<PathBuf>();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event {
            for path in event.paths {
                // The receiver is gone once the app shuts down; nothing to tell then.
                let _ = sender.send(path);
            }
        }
    })?;
    watcher.watch(&folder, RecursiveMode::NonRecursive)?;
    std::thread::Builder::new()
        .name("rb-config-watch".to_owned())
        .spawn(move || debounce(&receiver, &on_change))
        .map_err(notify::Error::io)?;
    Ok(ConfigWatch { _watcher: watcher })
}

fn debounce(receiver: &mpsc::Receiver<PathBuf>, on_change: &dyn Fn()) {
    while let Ok(path) = receiver.recv() {
        let mut relevant = is_config_file(&path);
        loop {
            match receiver.recv_timeout(DEBOUNCE) {
                Ok(path) => relevant |= is_config_file(&path),
                Err(mpsc::RecvTimeoutError::Timeout) => break,
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        if relevant {
            on_change();
        }
    }
}

fn is_config_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == CONFIG_FILE || name == LOCAL_CONFIG_FILE)
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
