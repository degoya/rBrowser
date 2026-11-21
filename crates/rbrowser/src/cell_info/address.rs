//! The address field of a decoupled view's label strip (RB-092-03): what it may open and when it
//! opens. A view whose navigation is decoupled (`Leader S U`, `breakpoints[].syncNavigation:
//! false`) keeps its own page; a click on its address piece, `view.edit_address` (`Leader S L`)
//! or the page menu opens a field in its strip, and Enter loads what was typed in that view only
//! ([`super::edit`]). The input is checked as the toolbar's address bar checks it - the core's
//! [`normalize_url`] completes a missing scheme the same way - and narrower: only `http`,
//! `https` and `file` open, and a path without a host opens under the environment in force (as a
//! bookmark's path does, [`rb_config::bookmarks::base_url`]). Pure; the catalogue gives every
//! word.

use rb_config::CellLabelSettings;
use rb_engine::ViewId;
use rb_i18n::{t, t_with};

use super::{CellInfo, PieceAction};
use crate::host::nav::{marker_view, normalize_url, scheme_of};

/// Opens the address field of a view (`{ "view"?: id }`, the active view when absent), or with
/// `url` loads it in that view only - the field's Enter. MCP loads a view's page with
/// `navigate { target: viewId }`.
pub const EDIT_ADDRESS: &str = "view.edit_address";
/// The setting that switches the field on (`ui.cellLabel.addressEditable`).
pub const SETTING: &str = "ui.cellLabel.addressEditable";
/// What a click on an editable address does, for the piece's tooltip.
const CLICK_KEY: &str = "cells.address.edit";

/// The schemes the field opens.
const SCHEMES: [&str; 3] = ["http", "https", "file"];

/// Why the field did not open or did not load what was typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `ui.cellLabel.addressEditable` is off.
    Off,
    /// The view's navigation follows the active view.
    Coupled,
    /// Nothing was typed.
    Empty,
    /// A path, with no environment and no start URL to open it under.
    NoBase,
    /// A scheme the field does not open (`javascript:`, `data:`, `chrome:`, `about:`, ...).
    Scheme(String),
    /// Not an address.
    Invalid,
}

impl Refusal {
    /// The i18n key of the status bar's sentence; placeholders `{label}` and, for a scheme,
    /// `{scheme}`.
    #[must_use]
    pub fn key(&self) -> &'static str {
        match self {
            Self::Off => "cells.address.refused.off",
            Self::Coupled => "cells.address.refused.coupled",
            Self::Empty => "cells.address.refused.empty",
            Self::NoBase => "cells.address.refused.noBase",
            Self::Scheme(_) => "cells.address.refused.scheme",
            Self::Invalid => "cells.address.refused.invalid",
        }
    }

    /// The placeholders of [`Self::key`] for the view labelled `label`.
    #[must_use]
    pub fn args(&self, label: &str) -> Vec<(&'static str, String)> {
        let mut args = vec![("label", label.to_owned())];
        if let Self::Scheme(scheme) = self {
            args.push(("scheme", scheme.clone()));
        }
        args
    }

    /// The sentence in the UI language, for the command's error.
    #[must_use]
    pub fn message(&self, label: &str) -> String {
        let args = self.args(label);
        let pairs: Vec<(&str, &str)> = args.iter().map(|(k, v)| (*k, v.as_str())).collect();
        t_with(self.key(), &pairs)
    }

    /// The setting the status bar links to: the switch, when it is off.
    #[must_use]
    pub fn settings_key(&self) -> Option<&'static str> {
        matches!(self, Self::Off).then_some(SETTING)
    }
}

/// Whether the field may open for a view whose navigation is `coupled`.
///
/// # Errors
///
/// [`Refusal::Off`] while the setting is off, [`Refusal::Coupled`] for a view that follows the
/// active view.
pub fn may_edit(settings: &CellLabelSettings, coupled: bool) -> Result<(), Refusal> {
    if !settings.address_editable {
        return Err(Refusal::Off);
    }
    if coupled {
        return Err(Refusal::Coupled);
    }
    Ok(())
}

/// The URL `input` opens: a path without a host (`/pricing`) under `base`, the URL of the
/// environment in force; anything else completed as the address bar completes it.
///
/// # Errors
///
/// Nothing typed, a path without `base`, a scheme other than `http`, `https` and `file`, or no
/// address at all.
pub fn resolve(input: &str, base: Option<&str>) -> Result<String, Refusal> {
    let text = input.trim();
    if text.is_empty() {
        return Err(Refusal::Empty);
    }
    if text.starts_with('/') && !text.starts_with("//") {
        let base = base.ok_or(Refusal::NoBase)?;
        return checked(&rb_config::bookmark_paths::url_in(base, text));
    }
    if let Some(scheme) = scheme_of(text) {
        let scheme = scheme.to_ascii_lowercase();
        if !SCHEMES.contains(&scheme.as_str()) {
            return Err(Refusal::Scheme(scheme));
        }
    }
    let url = normalize_url(text).map_err(|_| Refusal::Invalid)?;
    checked(&url)
}

/// `url` parsed and written as the address bar writes it (`new URL(...).href`): one of
/// [`SCHEMES`], a host for `http` and `https`.
fn checked(url: &str) -> Result<String, Refusal> {
    let parsed = url::Url::parse(url).map_err(|_| Refusal::Invalid)?;
    let scheme = parsed.scheme();
    if !SCHEMES.contains(&scheme) {
        return Err(Refusal::Scheme(scheme.to_owned()));
    }
    if scheme != "file" && parsed.host_str().is_none_or(str::is_empty) {
        return Err(Refusal::Invalid);
    }
    Ok(parsed.into())
}

/// Windows' virtual key codes of the keys the field answers (CEF reports them on every platform).
const VK_RETURN: i32 = 0x0D;
const VK_ESCAPE: i32 = 0x1B;

/// What a key does in the field.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKey {
    /// Enter: load what was typed (`view.edit_address` with `url`).
    Submit,
    /// Escape: close the field, the view keeps its page.
    Cancel,
}

/// What the key `windows_key_code` does in the field when it goes `down`; `None` for every other
/// key, which types.
#[must_use]
pub fn field_key(windows_key_code: i32, down: bool) -> Option<FieldKey> {
    match (down, windows_key_code) {
        (true, VK_RETURN) => Some(FieldKey::Submit),
        (true, VK_ESCAPE) => Some(FieldKey::Cancel),
        _ => None,
    }
}

/// What the field shows when it opens: the view's address, nothing while it loaded none yet.
#[must_use]
pub fn field_text(url: &str) -> &str {
    if marker_view(url).is_some() { "" } else { url }
}

/// The address pieces of the `decoupled` views open the field on a click, and their tooltip
/// says so; with the setting off they stay as they are (the click makes the view active).
pub fn mark_editable(infos: &mut [CellInfo], decoupled: &[ViewId], settings: &CellLabelSettings) {
    if !settings.address_editable {
        return;
    }
    let click = t_with("cells.info.click", &[("action", &t(CLICK_KEY))]);
    for info in infos
        .iter_mut()
        .filter(|info| decoupled.contains(&info.view))
    {
        if let Some(piece) = &mut info.address {
            piece.action = PieceAction::EditAddress;
            piece.tooltip.push('\n');
            piece.tooltip.push_str(&click);
        }
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
