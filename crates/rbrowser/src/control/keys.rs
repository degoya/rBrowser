//! The keyboard: every key-down CEF reports for a browser view or the shell (`OnPreKeyEvent`)
//! goes through the leader state machine (RB-010-17). A key that is not rBrowser's reaches the
//! page, DevTools or the OS untouched; the which-key overlay follows the chord.

use std::time::{Duration, Instant};

use rb_commands::keymap::{key_from_character, modifiers_from_event_flags, shortcut_from_cef};
use rb_commands::{Binding, Focus, KeyOutcome, Shortcut};
use rb_ipc::CoreToShell;

use super::Core;
use crate::cdp::lock;

/// How often a pending leader is checked for its timeout.
const TICK: Duration = Duration::from_millis(100);

/// One key-down as CEF describes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyDown {
    /// `cef_event_flags_t`.
    pub flags: u32,
    /// The Windows virtual-key code.
    pub windows_key_code: u32,
    /// The character the key types with the modifiers held (AltGr+9 -> `]` on German).
    pub character: Option<char>,
    /// The character it types without modifiers.
    pub unmodified_character: Option<char>,
}

impl KeyDown {
    /// The key-down of a CEF key event, from its fields: `down` for `RAWKEYDOWN` and `KEYDOWN`
    /// (every other event is `None`), `modifiers`, the Windows key code (a negative one is 0) and
    /// the two UTF-16 characters (a lone surrogate is none). Shared by the browsers' keyboard
    /// handlers and the main window's fallback (`cef_app/keys.rs`).
    pub fn from_event(
        down: bool,
        modifiers: u32,
        windows_key_code: i32,
        character: u16,
        unmodified_character: u16,
    ) -> Option<Self> {
        down.then(|| Self {
            flags: modifiers,
            windows_key_code: u32::try_from(windows_key_code).unwrap_or(0),
            character: char::from_u32(u32::from(character)),
            unmodified_character: char::from_u32(u32::from(unmodified_character)),
        })
    }
}

impl Core {
    /// A key-down in `focus`; `true` when rBrowser takes it (CEF then stops it).
    pub fn on_key_down(&self, focus: Focus, key: KeyDown, now: Instant) -> bool {
        let mut chords = lock(&self.chords);
        let Some(shortcut) = shortcut_of(key, chords.is_pending()) else {
            // A modifier alone, or a key rBrowser never binds.
            return false;
        };
        let pending = chords.is_pending();
        let outcome = chords.on_key(&lock(&self.registry), focus, shortcut, now);
        drop(chords);
        match outcome {
            // A project's breakpoint key, or one with the new-view modifier (RB-010-16); a
            // bookmark's own key (RB-070-06).
            KeyOutcome::PassThrough
                if !pending && matches!(focus, Focus::View | Focus::Shell | Focus::Board) =>
            {
                crate::breakpoint_keys::on_unbound_key(self, shortcut, |binding: &Binding| {
                    lock(&self.registry).resolve(binding).cloned()
                }) || crate::bookmarks::on_unbound_key(self, shortcut)
            }
            KeyOutcome::PassThrough => false,
            KeyOutcome::Run(id) => {
                self.send(&CoreToShell::WhichKeyHide);
                self.spawn_command(id);
                true
            }
            KeyOutcome::Show(which) => {
                self.send(&CoreToShell::WhichKey(which));
                true
            }
            KeyOutcome::Hide => {
                self.send(&CoreToShell::WhichKeyHide);
                true
            }
        }
    }

    /// Hides the overlay once a pending leader timed out at `now`.
    pub fn on_tick(&self, now: Instant) {
        let expired = lock(&self.chords).on_tick(now);
        if expired.is_some() {
            self.send(&CoreToShell::WhichKeyHide);
        }
    }

    /// Checks the leader's timeout every [`TICK`] while the core lives.
    pub(super) fn spawn_ticker(&self) {
        let me = self.me.clone();
        self.runtime.spawn(async move {
            let mut interval = tokio::time::interval(TICK);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let Some(core) = me.upgrade() else {
                    return;
                };
                core.on_tick(Instant::now());
            }
        });
    }
}

/// The shortcut of `key`. After the leader the typed character counts (`Leader ]` where `]`
/// needs AltGr); otherwise the virtual key, with the unmodified character for the keys that
/// differ per layout.
fn shortcut_of(key: KeyDown, pending: bool) -> Option<Shortcut> {
    if pending && let Some(typed) = key.character.and_then(key_from_character) {
        return Some(Shortcut {
            modifiers: modifiers_from_event_flags(key.flags),
            key: typed,
        });
    }
    shortcut_from_cef(key.flags, key.windows_key_code, key.unmodified_character)
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
