//! The overlay's colours (RB-090-58): the ruler bar, its ticks and numbers, the guides, the
//! column overlay's fill and edge, and the baseline grid.
//!
//! Each is a setting (`colors.<element>`, user, project and local scope, Settings > DevTools &
//! Debugging > Rulers) and a personal override like the rest of the state (the panel, MCP
//! `rulers_set`). A colour is `#rrggbb` or `#rrggbbaa` - the two last digits the opacity - and
//! nothing else, so a value can go into the page script's CSS as it is.

use serde::{Deserialize, Serialize};

use crate::model::Invalid;

/// The schema's `pattern` of a colour (`settings.schema.json`).
pub const COLOR_PATTERN: &str = "^#([0-9A-Fa-f]{6}|[0-9A-Fa-f]{8})$";

/// Whether `text` is a colour: `#` and six or eight hex digits.
#[must_use]
pub fn is_color(text: &str) -> bool {
    text.strip_prefix('#').is_some_and(|digits| {
        (digits.len() == 6 || digits.len() == 8) && digits.chars().all(|c| c.is_ascii_hexdigit())
    })
}

/// The colours the views draw in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Colors {
    /// The ruler bar's background (and its corner).
    pub ruler: String,
    /// The ruler's ticks and numbers.
    pub ticks: String,
    /// The guides and their handles.
    pub guide: String,
    /// The column overlay's columns.
    pub column_fill: String,
    /// The columns' left and right edge.
    pub column_edge: String,
    /// The baseline grid's lines.
    pub baseline: String,
}

impl Default for Colors {
    /// The colours the overlay drew before they could be set: a light, nearly opaque ruler bar,
    /// blue guides, pink columns without an edge, a light blue baseline grid.
    fn default() -> Self {
        Self {
            ruler: "#f6f7f9f0".to_owned(),
            ticks: "#4a4f5c".to_owned(),
            guide: "#00a2ff".to_owned(),
            column_fill: "#ff40811f".to_owned(),
            column_edge: "#ff408100".to_owned(),
            baseline: "#00a2ff59".to_owned(),
        }
    }
}

/// The person's colours; `None` leaves the shared one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct ColorOverrides {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruler: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticks: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guide: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_fill: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_edge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline: Option<String>,
}

impl ColorOverrides {
    fn fields(&self) -> [(&Option<String>, Invalid); 6] {
        [
            (&self.ruler, "colors.ruler"),
            (&self.ticks, "colors.ticks"),
            (&self.guide, "colors.guide"),
            (&self.column_fill, "colors.columnFill"),
            (&self.column_edge, "colors.columnEdge"),
            (&self.baseline, "colors.baseline"),
        ]
    }

    /// Whether no colour is overridden.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields().iter().all(|(value, _)| value.is_none())
    }

    /// # Errors
    ///
    /// The first colour that is none.
    pub fn check(&self) -> Result<(), Invalid> {
        for (value, field) in self.fields() {
            if value.as_deref().is_some_and(|text| !is_color(text)) {
                return Err(field);
            }
        }
        Ok(())
    }

    /// Takes over every colour `other` sets.
    pub fn merge(&mut self, other: Self) {
        for (mine, theirs) in [
            (&mut self.ruler, other.ruler),
            (&mut self.ticks, other.ticks),
            (&mut self.guide, other.guide),
            (&mut self.column_fill, other.column_fill),
            (&mut self.column_edge, other.column_edge),
            (&mut self.baseline, other.baseline),
        ] {
            if theirs.is_some() {
                *mine = theirs;
            }
        }
    }

    /// `shared` with these colours over it.
    #[must_use]
    pub fn over(&self, shared: &Colors) -> Colors {
        let pick =
            |own: &Option<String>, shared: &String| own.clone().unwrap_or_else(|| shared.clone());
        Colors {
            ruler: pick(&self.ruler, &shared.ruler),
            ticks: pick(&self.ticks, &shared.ticks),
            guide: pick(&self.guide, &shared.guide),
            column_fill: pick(&self.column_fill, &shared.column_fill),
            column_edge: pick(&self.column_edge, &shared.column_edge),
            baseline: pick(&self.baseline, &shared.baseline),
        }
    }
}

#[cfg(test)]
#[path = "colors_tests.rs"]
mod tests;
