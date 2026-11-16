//! The user settings of wave 0.1-w1b about starting and looking at the grid (contract 0.1-w1b):
//! `startup` (RB-010-11), `grid` and `focusMode` (RB-010-12).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Zoom;

/// `startup`: what happens when rBrowser starts or a project opens (RB-010-11).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct StartupSettings {
    /// Started without a project (and none found upwards): open the newest of
    /// `recentProjects`.
    pub open_last_project: bool,
    /// The window takes the focus when a project opens or a second call hands it a URL;
    /// `--background` overrides it for one call.
    pub focus_on_open: bool,
}

impl Default for StartupSettings {
    fn default() -> Self {
        Self {
            open_last_project: false,
            focus_on_open: true,
        }
    }
}

/// `grid`: the zoom of the grid area (RB-010-12). Panning the grid with Space + drag comes with
/// the canvas layout (RB-080-04).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct GridViewSettings {
    /// After a layout change (a layout preset, focus mode, the project opening or its config
    /// changing), views zoomed by hand go back to their cell's zoom; off, they keep it. An
    /// environment change leaves the zoom alone.
    pub fit_to_window: bool,
    /// The zoom of the cells rBrowser lays out itself (no `layout.cells` in the config, the
    /// focus-mode view): `"fit"` or a factor `0.25`..=`1`; cells of the config keep their own.
    pub default_zoom: Zoom,
}

impl Default for GridViewSettings {
    fn default() -> Self {
        Self {
            fit_to_window: true,
            default_zoom: Zoom::Fit,
        }
    }
}

/// `focusMode`: the one large view of a `focus` preset (RB-010-12).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct FocusModeSettings {
    /// Left and right arrow switch the breakpoint while the shell has the focus.
    pub arrow_keys: bool,
    /// A page loading in the focus view (each arrow key opens the next breakpoint's view) leaves
    /// the keyboard focus where it is, so the toolbar's next arrow key steps again. Off: the
    /// view takes the focus, as a page does after a navigation.
    pub keep_focus: bool,
}

impl Default for FocusModeSettings {
    fn default() -> Self {
        Self {
            arrow_keys: true,
            keep_focus: true,
        }
    }
}

/// `settings`: the settings window itself (RB-080-42).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsWindowSettings {
    /// The JSON view colours keys, strings, numbers, booleans, `null` and punctuation, in both
    /// themes; off, the text is one colour. Copy delivers plain JSON either way.
    pub json_highlight: bool,
    /// Every setting prints its key (`updates.channel`) under its title. Off, the key is in the
    /// title's tooltip and behind the copy button that appears on the row; the search finds a
    /// setting by its key either way (RB-080-70).
    pub show_keys: bool,
    /// A category with more than one section shows one section per tab; the search, "Only
    /// changed" and a link to a setting reach every tab. Off, every section of a category stands
    /// on one page (RB-080-71).
    pub section_tabs: bool,
}

impl Default for SettingsWindowSettings {
    fn default() -> Self {
        Self {
            json_highlight: true,
            show_keys: false,
            section_tabs: true,
        }
    }
}

/// `bookmarks`: how "Bookmark this page" stores a page (RB-080-48). The bookmarks themselves are
/// the project's (`bookmarks[]` of `config.json`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct BookmarkSettings {
    /// A page that lies under an environment of the project is bookmarked as its path
    /// (`/shop/cart?x=1#top`), so the bookmark opens on whichever environment is in effect; a
    /// page of a foreign host stays an absolute URL. Off, every new bookmark is an absolute URL.
    pub follow_environment: bool,
}

impl Default for BookmarkSettings {
    fn default() -> Self {
        Self {
            follow_environment: true,
        }
    }
}
