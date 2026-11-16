//! The shell follows Windows' text size (RB-090-26, WCAG 1.4.4): *Settings > Accessibility >
//! Text size*, `TextScaleFactor` from 100 to 225 %, read at the start and again whenever Windows
//! changes it (`win.rs`; elsewhere the size stays at 100 %). `ui.followWindowsTextSize`
//! (on by default) switches it off.
//!
//! | Part | How it follows |
//! | --- | --- |
//! | The shell's pages (toolbar, status bar, settings, palette, panels page, canvas board) | the root font size, so every `rem` of Tailwind and Nuxt UI grows (`ui/src/theme/textScale.ts`); the page zoom stays at none (`cef_app/zoom.rs`) |
//! | The native heights: toolbar row, status bar, label strip, breakpoint bar line and font, the folded dock | [`dip`] over their sizes at 100 % |
//! | The palette and the message history | their preferred size, still inside the window (`page_window.rs`) |
//! | The pages in the views | never: their zoom is the page zoom's (RB-090-20) |
//!
//! A new percentage is announced once ([`announce`]): the event `textScale` to every page of
//! the shell, the window's box asks the toolbar and the status bar for their heights again, and
//! the grid (or the layout mode's places) is sent again with its views kept, for the strips'
//! heights - the popped-out views' windows included.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use rb_ipc::{CoreToShell, ShellToCore};

use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;

#[cfg(windows)]
mod win;
#[cfg(windows)]
use win as system;

/// Outside Windows the text size is not read (macOS's later, RB-090-26): 100 %.
#[cfg(not(windows))]
mod system {
    /// Nothing to read.
    pub fn read() -> Option<u32> {
        None
    }

    /// Nothing to watch.
    pub fn watch(on_change: impl Fn(Option<u32>) + Send + 'static) {
        let _ = on_change;
    }
}

/// 100 %: the size every native height and the shell's CSS are drawn for.
pub const BASE_PERCENT: u32 = 100;
/// Windows' largest text size.
pub const MAX_PERCENT: u32 = 225;
/// The setting that switches following off.
pub const SETTING_KEY: &str = "ui.followWindowsTextSize";

/// The sizes at 100 % that follow the text size, in DIP (the footer's font in px).
const TOOLBAR_ROW: u32 = crate::shell::SHELL_HEIGHT;
const STATUS_BAR: u32 = crate::shell::STATUS_BAR_HEIGHT;
const LABEL: u32 = crate::host::LABEL_HEIGHT;
const FOOTER_LINE: u32 = crate::view_footer::FOOTER_HEIGHT;
/// The breakpoint bar's font size in px (`cef_app/cell_footers.rs`).
pub const FOOTER_FONT_PX: u32 = 11;
const DOCK_STRIP: (u32, u32) = (
    crate::panel_dock::collapse::STRIP_WIDTH,
    crate::panel_dock::collapse::STRIP_HEIGHT,
);

/// What Windows says and whether the shell follows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextScale {
    system: u32,
    follow: bool,
}

impl TextScale {
    /// Before Windows was read: 100 %, following.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            system: BASE_PERCENT,
            follow: true,
        }
    }

    /// With Windows' `TextScaleFactor` as read (`None`: absent or unreadable).
    #[must_use]
    pub fn with_system(self, raw: Option<u32>) -> Self {
        Self {
            system: clamp_percent(raw),
            ..self
        }
    }

    /// With `ui.followWindowsTextSize`.
    #[must_use]
    pub fn with_follow(self, follow: bool) -> Self {
        Self { follow, ..self }
    }

    /// The percentage in effect: Windows' while following, else 100 %.
    #[must_use]
    pub fn percent(self) -> u32 {
        if self.follow {
            self.system
        } else {
            BASE_PERCENT
        }
    }
}

impl Default for TextScale {
    fn default() -> Self {
        Self::new()
    }
}

/// Windows' `TextScaleFactor` as the shell takes it: absent is 100 %, a value outside Windows'
/// range is held to 100..=225.
#[must_use]
pub fn clamp_percent(raw: Option<u32>) -> u32 {
    raw.map_or(BASE_PERCENT, |percent| {
        percent.clamp(BASE_PERCENT, MAX_PERCENT)
    })
}

/// `size` (at 100 %) at `percent`, rounded to the nearest whole unit.
#[must_use]
pub fn scale(size: u32, percent: u32) -> u32 {
    let scaled = u64::from(size) * u64::from(percent) + 50;
    u32::try_from(scaled / 100).unwrap_or(u32::MAX)
}

/// [`scale`] for a signed size; a negative size is none.
#[must_use]
pub fn scale_i32(size: i32, percent: u32) -> i32 {
    let size = u32::try_from(size).unwrap_or(0);
    i32::try_from(scale(size, percent)).unwrap_or(i32::MAX)
}

/// The native sizes at `percent`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Heights {
    /// One toolbar row (the shell view's height per row).
    pub toolbar_row: u32,
    /// The status bar.
    pub status_bar: u32,
    /// The label strip above each view.
    pub label: u32,
    /// One line of the breakpoint bar below each view.
    pub footer_line: u32,
    /// The breakpoint bar's font, px.
    pub footer_font_px: u32,
    /// The folded dock: its icon column's width, its header's height.
    pub dock_strip: (u32, u32),
}

/// The native sizes at `percent`.
#[must_use]
pub fn heights(percent: u32) -> Heights {
    Heights {
        toolbar_row: scale(TOOLBAR_ROW, percent),
        status_bar: scale(STATUS_BAR, percent),
        label: scale(LABEL, percent),
        footer_line: scale(FOOTER_LINE, percent),
        footer_font_px: scale(FOOTER_FONT_PX, percent),
        dock_strip: (scale(DOCK_STRIP.0, percent), scale(DOCK_STRIP.1, percent)),
    }
}

/// Applies `change` to `scale`; the new percentage when it differs from the old one.
pub fn change(scale: &mut TextScale, apply: impl FnOnce(TextScale) -> TextScale) -> Option<u32> {
    let before = scale.percent();
    *scale = apply(*scale);
    let after = scale.percent();
    (after != before).then_some(after)
}

/// What Windows says and the setting, for the whole app.
static STATE: Mutex<TextScale> = Mutex::new(TextScale::new());
/// [`STATE`]'s percentage, read by every layout without the lock.
static PERCENT: AtomicU32 = AtomicU32::new(BASE_PERCENT);

/// The percentage in effect now.
#[must_use]
pub fn percent() -> u32 {
    PERCENT.load(Ordering::Acquire)
}

/// `size` (at 100 %) at the percentage in effect now.
#[must_use]
pub fn dip(size: u32) -> u32 {
    scale(size, percent())
}

/// [`dip`] for a signed size.
#[must_use]
pub fn dip_i32(size: i32) -> i32 {
    scale_i32(size, percent())
}

/// The core is up: Windows' text size is read, followed from now on, and the shell takes it.
pub fn on_start(core: &Core) {
    let follow = core.user_settings().ui.follow_windows_text_size;
    update(core, |scale| {
        scale.with_follow(follow).with_system(system::read())
    });
    let Some(core) = core.arc() else {
        return;
    };
    let core = std::sync::Arc::downgrade(&core);
    system::watch(move |raw| {
        if let Some(core) = core.upgrade() {
            update(&core, |scale| scale.with_system(raw));
        }
    });
}

/// The user settings changed: `ui.followWindowsTextSize` may have.
pub fn on_user_settings_changed(core: &Core) {
    let follow = core.user_settings().ui.follow_windows_text_size;
    update(core, |scale| scale.with_follow(follow));
}

/// Changes the app's [`TextScale`] and announces a new percentage. The announcement runs under
/// the same lock as the change: two changes at once (Windows' watcher and the setting) are
/// announced in the order they took effect, so the pages and the native heights never end on
/// different percentages. Nothing on the announcement's way takes this lock again.
fn update(core: &Core, apply: impl FnOnce(TextScale) -> TextScale) {
    let mut state = lock(&STATE);
    if let Some(percent) = change(&mut state, apply) {
        PERCENT.store(percent, Ordering::Release);
        tracing::info!(percent, "the shell follows Windows' text size");
        announce(core, percent);
    }
}

/// `percent` is in effect: the pages of the shell take it, the window lays the toolbar and the
/// status bar out again, and the grid is sent again for the strips' heights.
pub fn announce(core: &Core, percent: u32) {
    core.send(&CoreToShell::TextScale {
        percent: wire(percent),
    });
    core.ui().relayout_shell();
    core.host().text_scale_changed();
}

/// `percent` as the event carries it.
fn wire(percent: u32) -> u16 {
    u16::try_from(percent.clamp(BASE_PERCENT, MAX_PERCENT)).unwrap_or(100)
}

/// Answers `getTextScale` with the percentage in effect; `None` for every other message.
pub fn ipc(_core: &Core, message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    matches!(message, ShellToCore::GetTextScale).then(|| {
        Ok(CoreToShell::TextScale {
            percent: wire(percent()),
        })
    })
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
