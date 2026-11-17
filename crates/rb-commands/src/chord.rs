//! The leader state machine and the which-key data.
//!
//! The host feeds every key-down it intercepts (CEF `OnPreKeyEvent`, mapped with
//! [`crate::keymap`]) to [`ChordMachine::on_key`] and acts on the [`KeyOutcome`]: `PassThrough`
//! lets the key reach the page, DevTools or the OS untouched; every other outcome consumes it.
//! After the leader the key is matched by the character it types, and Shift and AltGr are
//! ignored, so `Leader ]` works as `AltGr+9` on a German layout. Pressing the leader twice runs
//! [`PALETTE_COMMAND`]. A pending leader waits until `Escape` or the next key; with a
//! `leader_timeout` set it expires after it, and the host calls [`ChordMachine::on_tick`] to
//! hide the overlay then (RB-080-18).

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::validate::group_of_key;
use crate::{Binding, Chord, CommandId, CommandRegistry, Group, Key, KeyContext, LEADER, Shortcut};

/// The command the double leader runs: the command palette.
pub const PALETTE_COMMAND: &str = "app.command_palette";

/// Where the keyboard focus is when a key arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    /// The shell UI.
    Shell,
    /// The canvas board behind the canvas's views (RB-080-94): the shell's keys, and the
    /// reservation list's context entries for `canvas-board` (`Ctrl+0`: 100 %).
    Board,
    /// A browser view of the grid.
    View,
    /// The DevTools panel.
    Devtools,
    /// An external Firefox or WebKit window: no global shortcuts.
    External,
}

/// The user settings the machine follows (`shortcuts.leaderTimeoutMs`,
/// `shortcuts.disableInDevtools`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChordSettings {
    /// How long the leader waits for the next key; `None` (the default): until `Escape` or a
    /// key.
    pub leader_timeout: Option<Duration>,
    /// rBrowser's shortcuts are off while DevTools have focus.
    pub disable_in_devtools: bool,
}

impl Default for ChordSettings {
    fn default() -> Self {
        Self {
            leader_timeout: None,
            disable_in_devtools: true,
        }
    }
}

/// What the host does with a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyOutcome {
    /// Not rBrowser's: the key reaches its target untouched.
    PassThrough,
    /// Run this command (consumes the key, hides the overlay).
    Run(CommandId),
    /// Show (or update) the which-key overlay (consumes the key).
    Show(WhichKey),
    /// Hide the overlay: cancelled, unknown key or timeout (consumes the key).
    Hide,
}

/// The which-key overlay: the keys typed after the leader and what each next key does. Sent to
/// the shell as `rb_ipc::CoreToShell::WhichKey`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhichKey {
    /// What was typed, in text form (`Ctrl+Shift+Space S`).
    pub typed: String,
    /// The possible next keys: groups first (in leader order), then commands by key.
    pub entries: Vec<WhichKeyEntry>,
}

/// One possible next key.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum WhichKeyEntry {
    /// The key opens a group.
    #[serde(rename_all = "camelCase")]
    Group {
        /// The key in text form.
        key: String,
        /// The group.
        group: Group,
        /// The i18n key of its title.
        title_key: String,
    },
    /// The key runs a command.
    #[serde(rename_all = "camelCase")]
    Command {
        /// The key in text form.
        key: String,
        /// The command.
        id: CommandId,
        /// The i18n key of its title.
        title_key: String,
    },
}

impl CommandRegistry {
    /// The which-key data after the leader and `typed` (empty: the root).
    pub fn which_key(&self, typed: &[Key]) -> WhichKey {
        let mut groups: Vec<(Group, WhichKeyEntry)> = Vec::new();
        let mut commands: Vec<(Key, WhichKeyEntry)> = Vec::new();
        for def in self.iter() {
            for binding in self.bindings(&def.id) {
                let Binding::Leader(chord) = binding else {
                    continue;
                };
                if chord.keys.len() <= typed.len() || !chord.keys.starts_with(typed) {
                    continue;
                }
                let next = chord.keys[typed.len()];
                if chord.keys.len() == typed.len() + 1 {
                    commands.push((
                        next,
                        WhichKeyEntry::Command {
                            key: next.to_string(),
                            id: def.id.clone(),
                            title_key: def.title_key.clone(),
                        },
                    ));
                } else if let Some(group) = group_of_key(next)
                    && !groups.iter().any(|(held, _)| *held == group)
                {
                    groups.push((
                        group,
                        WhichKeyEntry::Group {
                            key: next.to_string(),
                            group,
                            title_key: group.title_key(),
                        },
                    ));
                }
            }
        }
        groups.sort_by_key(|(group, _)| Group::ALL.iter().position(|g| g == group));
        commands.sort_by_key(|(key, _)| *key);
        WhichKey {
            typed: Chord {
                keys: typed.to_vec(),
            }
            .to_string(),
            entries: groups
                .into_iter()
                .map(|(_, entry)| entry)
                .chain(commands.into_iter().map(|(_, entry)| entry))
                .collect(),
        }
    }
}

struct Pending {
    keys: Vec<Key>,
    since: Instant,
}

/// The leader state machine. One per window; it holds no registry, so the host passes the
/// current one with each key.
#[derive(Default)]
pub struct ChordMachine {
    settings: ChordSettings,
    pending: Option<Pending>,
}

impl ChordMachine {
    /// A machine with `settings`.
    pub fn new(settings: ChordSettings) -> Self {
        Self {
            settings,
            pending: None,
        }
    }

    /// Takes changed settings; a pending leader stays.
    pub fn set_settings(&mut self, settings: ChordSettings) {
        self.settings = settings;
    }

    /// Whether a leader waits for its next key.
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    /// One key-down with its modifiers, at `now`.
    pub fn on_key(
        &mut self,
        registry: &CommandRegistry,
        focus: Focus,
        key: Shortcut,
        now: Instant,
    ) -> KeyOutcome {
        let off = focus == Focus::External
            || (focus == Focus::Devtools && self.settings.disable_in_devtools);
        if off {
            self.pending = None;
            return KeyOutcome::PassThrough;
        }
        let pending = self.pending.take().filter(|p| !self.expired(p, now));
        let Some(pending) = pending else {
            if focus == Focus::Board
                && let Some(id) = registry.context_command(&key, KeyContext::CanvasBoard)
            {
                return KeyOutcome::Run(id.clone());
            }
            return self.idle_key(registry, key, now);
        };
        if key == LEADER {
            if pending.keys.is_empty() {
                return CommandId::new(PALETTE_COMMAND)
                    .ok()
                    .filter(|id| registry.get(id).is_some())
                    .map_or(KeyOutcome::Hide, KeyOutcome::Run);
            }
            return self.idle_key(registry, key, now);
        }
        let m = key.modifiers;
        let altgr = m.ctrl && m.alt;
        if m.meta || (m.ctrl && !altgr) || (m.alt && !altgr) {
            // A combination of its own ends the chord and counts as a fresh key.
            return self.idle_key(registry, key, now);
        }
        if key.key == Key::Escape {
            return KeyOutcome::Hide;
        }
        let mut keys = pending.keys;
        keys.push(key.key);
        let chord = Binding::Leader(Chord { keys: keys.clone() });
        if let Some(id) = registry.resolve(&chord) {
            return KeyOutcome::Run(id.clone());
        }
        if registry.is_chord_prefix(&keys) {
            let which = registry.which_key(&keys);
            self.pending = Some(Pending { keys, since: now });
            return KeyOutcome::Show(which);
        }
        KeyOutcome::Hide
    }

    fn idle_key(&mut self, registry: &CommandRegistry, key: Shortcut, now: Instant) -> KeyOutcome {
        if key == LEADER {
            self.pending = Some(Pending {
                keys: Vec::new(),
                since: now,
            });
            return KeyOutcome::Show(registry.which_key(&[]));
        }
        match registry.resolve(&Binding::Direct(key)) {
            Some(id) => KeyOutcome::Run(id.clone()),
            None => KeyOutcome::PassThrough,
        }
    }

    /// Whether `pending` outlived the timeout at `now`; never without one.
    fn expired(&self, pending: &Pending, now: Instant) -> bool {
        self.settings
            .leader_timeout
            .is_some_and(|timeout| now.duration_since(pending.since) > timeout)
    }

    /// Expires a pending leader at `now`: `Some(Hide)` once, when it timed out; never without a
    /// timeout.
    pub fn on_tick(&mut self, now: Instant) -> Option<KeyOutcome> {
        let expired = self.pending.as_ref().is_some_and(|p| self.expired(p, now));
        if expired {
            self.pending = None;
            return Some(KeyOutcome::Hide);
        }
        None
    }
}

#[cfg(test)]
#[path = "chord_tests.rs"]
mod tests;
