//! The colours of rBrowser's native parts in each theme (RB-080-95): the window ground behind
//! the grid and the label strips, the strips' text, the dimmed text of an engine that cannot
//! run, and the native menus. The values are Nuxt UI's `slate` neutral, the shell's
//! (`ui/vite.config.ts`), so a strip reads as part of the toolbar above it: `bg-default`,
//! `text-default`, `text-muted`, `bg-elevated`. Every text pair reaches WCAG AA (4.5 : 1) in both
//! themes, the leader's mark included, and the mark stands 3 : 1 off the plain strip (the tests
//! hold it). No CEF in this file.

use rb_config::Theme;

/// A theme as it is drawn: `system` resolved to what Windows uses for apps.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resolved {
    /// Light.
    Light,
    /// Dark.
    Dark,
}

impl Resolved {
    /// Whether this is the dark theme (the title bar's dark mode).
    #[must_use]
    pub fn is_dark(self) -> bool {
        self == Resolved::Dark
    }
}

/// `setting` as it is drawn: `system` follows `system_dark` (Windows' app mode).
#[must_use]
pub fn resolve(setting: Theme, system_dark: bool) -> Resolved {
    match setting {
        Theme::Light => Resolved::Light,
        Theme::Dark => Resolved::Dark,
        Theme::System if system_dark => Resolved::Dark,
        Theme::System => Resolved::Light,
    }
}

/// The native colours of one theme, each `0xrrggbb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativePalette {
    /// The ground of the windows, the grid and the label strips.
    pub ground: u32,
    /// The text of the label strips and buttons.
    pub text: u32,
    /// Dimmed text that must still be read: an engine badge whose engine cannot run.
    pub muted: u32,
    /// The ground of a native menu.
    pub menu_ground: u32,
    /// A menu row's text.
    pub menu_text: u32,
    /// The ground of the row under the pointer.
    pub menu_hover_ground: u32,
    /// The text of the row under the pointer.
    pub menu_hover_text: u32,
    /// The lines between groups of rows and around a menu.
    pub menu_line: u32,
    /// The ground of the leader's label strip (`ui.highlightActiveView`): the shell's primary,
    /// solid, so the strip stands 3 : 1 off its neighbours (RB-090-22).
    pub mark_ground: u32,
    /// The text of the leader's label strip, on [`NativePalette::mark_ground`].
    pub mark_text: u32,
}

/// Light: white ground, slate 700 text (`bg-default`, `text-default`).
pub const LIGHT: NativePalette = NativePalette {
    ground: 0x00ff_ffff,
    text: 0x0033_4155,
    muted: 0x0064_748b,
    menu_ground: 0x00ff_ffff,
    menu_text: 0x0033_4155,
    menu_hover_ground: 0x00f1_f5f9,
    menu_hover_text: 0x000f_172a,
    menu_line: 0x00e2_e8f0,
    // Teal 800, the light shell's primary (`ui/src/assets/main.css`); white on it, as on a
    // solid primary button.
    mark_ground: 0x0000_5f5a,
    mark_text: 0x00ff_ffff,
};

/// Dark: slate 900 ground, slate 200 text; menus on slate 800 (`bg-elevated`).
pub const DARK: NativePalette = NativePalette {
    ground: 0x000f_172a,
    text: 0x00e2_e8f0,
    muted: 0x0094_a3b8,
    menu_ground: 0x001e_293b,
    menu_text: 0x00e2_e8f0,
    menu_hover_ground: 0x0033_4155,
    menu_hover_text: 0x00ff_ffff,
    menu_line: 0x0033_4155,
    // Teal 400, the dark shell's primary; the ground's slate 900 on it.
    mark_ground: 0x0000_d5be,
    mark_text: 0x000f_172a,
};

/// The sign before the leader's label (RB-090-22): the active view is told by more than its
/// colour - a black circle and a space, `\u{25cf} `.
pub const ACTIVE_SIGN: &str = "\u{25cf} ";

/// The text a label strip shows for `label`: with [`ACTIVE_SIGN`] before it when its view
/// `leads`, without when not. `label` may carry the sign already (the strip's text as it is).
#[must_use]
pub fn marked(label: &str, leads: bool) -> String {
    let plain = label.strip_prefix(ACTIVE_SIGN).unwrap_or(label);
    if leads {
        format!("{ACTIVE_SIGN}{plain}")
    } else {
        plain.to_owned()
    }
}

/// The colours of `theme`.
#[must_use]
pub fn palette(theme: Resolved) -> NativePalette {
    match theme {
        Resolved::Light => LIGHT,
        Resolved::Dark => DARK,
    }
}

/// `0xrrggbb` as opaque ARGB, as CEF's views take colours.
#[must_use]
pub fn opaque(rgb: u32) -> u32 {
    0xff00_0000 | (rgb & 0x00ff_ffff)
}

/// The ARGB colour `over` laid on the opaque `ground` (`0xrrggbb`): what the eye sees, `0xrrggbb`.
#[must_use]
pub fn composite(over: u32, ground: u32) -> u32 {
    let alpha = (over >> 24) & 0xff;
    let channel = |shift: u32| {
        let top = (over >> shift) & 0xff;
        let bottom = (ground >> shift) & 0xff;
        (top * alpha + bottom * (255 - alpha) + 127) / 255
    };
    (channel(16) << 16) | (channel(8) << 8) | channel(0)
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
