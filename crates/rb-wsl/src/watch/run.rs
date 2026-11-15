//! `rbrowser-wsl watch <path>`: the helper's loop. After the handshake rBrowser sends `watch`
//! with the [`WatchRules`]; the helper watches the tree with inotify (folder by folder, ignored
//! folders left out), debounces and writes each batch as `changes` (or `rescan` after lost
//! events). A new `watch` replaces the rules; `ping`, `shutdown` and the end of the input work
//! as in every subcommand.
//!
//! Lines and file events meet in one channel: a thread reads the input, notify's thread sends
//! the events, and only this loop writes the output.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::{Config, RecommendedWatcher, Watcher as _};

use super::WatchRules;
use super::batch::{Batch, Debouncer};
use super::filter::Filter;
use super::tree::{Seen, TreeError, TreeMode, TreeWatch};
use crate::protocol::{ErrorCode, MAX_LINE, Message, decode};
use crate::serve::{Line, Served, bad_request, handshake, read_line, send, skip_line};

/// What the loop waits for.
enum Input {
    Line(Line),
    Fs(notify::Result<notify::Event>),
}

/// A running watch.
struct Running {
    tree: TreeWatch<RecommendedWatcher>,
    debounce: Debouncer,
}

/// Serves `watch` of the folder `root` (absolute) for a helper of version `helper_version`.
///
/// # Errors
///
/// When writing the output fails.
pub fn serve_watch<R, W>(
    mut input: R,
    mut output: W,
    helper_version: &str,
    root: &Path,
) -> io::Result<Served>
where
    R: BufRead + Send + 'static,
    W: Write,
{
    if let Some(failed) = handshake(&mut input, &mut output, helper_version)? {
        return Ok(failed);
    }
    let (events, inbox) = mpsc::channel::<Input>();
    spawn_reader(input, events.clone())?;
    let mut running: Option<Running> = None;
    loop {
        let deadline = running.as_ref().and_then(|run| run.debounce.deadline());
        let next = match deadline {
            Some(deadline) => {
                match inbox.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                    Ok(next) => Some(next),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(Served::Closed),
                }
            }
            None => match inbox.recv() {
                Ok(next) => Some(next),
                Err(_) => return Ok(Served::Closed),
            },
        };
        match next {
            None => {}
            Some(Input::Line(Line::End)) => return Ok(Served::Closed),
            Some(Input::Line(Line::TooLong)) => {
                send(&mut output, &bad_request("a line is too long"))?;
            }
            Some(Input::Line(Line::Text(line))) => match decode(&line) {
                Ok(Message::Ping) => send(&mut output, &Message::Pong)?,
                Ok(Message::Shutdown) => return Ok(Served::Shutdown),
                Ok(Message::Watch { rules }) => {
                    // The old watch ends before the new one starts (its watcher is dropped).
                    running = None;
                    match start(root, &rules, &events) {
                        Ok(run) => {
                            let folders = u64::try_from(run.tree.folders()).unwrap_or(u64::MAX);
                            running = Some(run);
                            send(&mut output, &Message::Watching { folders })?;
                        }
                        Err(reply) => send(&mut output, &reply)?,
                    }
                }
                Ok(other) => send(
                    &mut output,
                    &bad_request(&format!("unexpected {}", other.kind())),
                )?,
                Err(error) => send(&mut output, &bad_request(&error.to_string()))?,
            },
            Some(Input::Fs(event)) => {
                if let Some(run) = running.as_mut()
                    && let Err(error) = on_event(run, event)
                {
                    running = None;
                    send(&mut output, &watch_failed(&error))?;
                }
            }
        }
        if let Some(run) = running.as_mut()
            && let Some(batch) = run.debounce.take(Instant::now())
        {
            let message = match batch {
                Batch::Changes(changes) => Message::Changes { changes },
                Batch::Rescan => Message::Rescan,
            };
            send(&mut output, &message)?;
        }
    }
}

/// Starts the watch of `root` with `rules`; the reply to send on failure.
fn start(root: &Path, rules: &WatchRules, events: &Sender<Input>) -> Result<Running, Message> {
    let filter = Filter::new(rules).map_err(|error| Message::Error {
        code: ErrorCode::BadRule,
        message: error.to_string(),
        protocol: None,
    })?;
    let sink = events.clone();
    let watcher = RecommendedWatcher::new(
        move |event| {
            // The loop is gone once the helper ends; nothing to tell then.
            let _ = sink.send(Input::Fs(event));
        },
        Config::default(),
    )
    .map_err(|error| Message::Error {
        code: ErrorCode::WatchFailed,
        message: error.to_string(),
        protocol: None,
    })?;
    let tree = TreeWatch::new(root, filter, watcher, TreeMode::PerFolder)
        .map_err(|error| watch_failed(&error))?;
    Ok(Running {
        tree,
        debounce: Debouncer::new(Duration::from_millis(rules.debounce_ms)),
    })
}

/// Feeds one notify result into the watch.
fn on_event(run: &mut Running, event: notify::Result<notify::Event>) -> Result<(), TreeError> {
    let now = Instant::now();
    let event = match event {
        Ok(event) => event,
        Err(error) => {
            // An error in the event stream: what changed is unknown.
            tracing::warn!(%error, "the watch reported an error");
            run.debounce.push_rescan(now);
            return Ok(());
        }
    };
    for seen in run.tree.on_event(&event)? {
        match seen {
            Seen::Change(change) => run.debounce.push(change, now),
            Seen::Rescan => {
                run.debounce.push_rescan(now);
                run.tree.rewalk()?;
            }
        }
    }
    Ok(())
}

fn watch_failed(error: &TreeError) -> Message {
    Message::Error {
        code: ErrorCode::WatchFailed,
        message: error.to_string(),
        protocol: None,
    }
}

/// Reads the input's lines into `events` on a thread of its own, until its end.
fn spawn_reader<R: BufRead + Send + 'static>(
    mut input: R,
    events: Sender<Input>,
) -> io::Result<()> {
    std::thread::Builder::new()
        .name("rbrowser-wsl-stdin".into())
        .spawn(move || {
            loop {
                let line = match read_line(&mut input, MAX_LINE) {
                    Ok(Line::TooLong) => match skip_line(&mut input) {
                        Ok(()) => Line::TooLong,
                        Err(_) => Line::End,
                    },
                    Ok(line) => line,
                    Err(_) => Line::End,
                };
                let end = line == Line::End;
                if events.send(Input::Line(line)).is_err() || end {
                    return;
                }
            }
        })
        .map(|_| ())
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
