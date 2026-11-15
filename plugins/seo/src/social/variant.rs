//! Which card a platform shows for a shared link (RB-080-99): every platform of
//! `data/social.json` carries its rule - the page's `twitter:card`, the share image's real size,
//! always one layout, or none published - with its source, and [`choose`] applies it to the
//! page's facts and says why. Where no rule is published, or the image's size is not known, both
//! variants are shown and marked unsure. The rules and their sources are in the job file
//! `docs/roadmap/jobs/080-99-social-previews-varianten.md`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The `twitter:card` value that asks for a large image.
pub const LARGE_CARD: &str = "summary_large_image";

/// The layout of a card.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// The image above the text.
    Large,
    /// A thumbnail beside the text.
    Small,
    /// No image: the text alone.
    Text,
}

impl Layout {
    /// The other layout with an image.
    #[must_use]
    pub fn other(self) -> Option<Self> {
        match self {
            Self::Large => Some(Self::Small),
            Self::Small => Some(Self::Large),
            Self::Text => None,
        }
    }
}

/// What a share image must be for the large card; a missing bound is no condition.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Size {
    pub min_width: Option<u32>,
    pub min_height: Option<u32>,
    /// The widest width / height (4 for 4:1).
    pub max_ratio: Option<u32>,
    pub max_bytes: Option<u64>,
    /// Wider than high.
    pub landscape: bool,
    /// Not square within this many per cent (|1 - width / height|).
    pub square_tolerance: Option<u32>,
}

/// How a platform picks its card.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Rule {
    /// Large when the page's `twitter:card` is `summary_large_image`.
    Card,
    /// Large when the share image meets every bound.
    Size(Size),
    /// Always this layout while there is an image.
    Always { layout: Layout },
    /// No published rule: both variants.
    Unverified,
}

/// The size of a share image as fetched; `None` when it is not known.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    pub width: u32,
    pub height: u32,
    pub bytes: u64,
}

/// Why a card has its layout: a code (`plugin.seo.panel.reasons.<code>`) and its parameters.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Reason {
    pub code: String,
    pub params: BTreeMap<String, String>,
}

impl Reason {
    fn new(code: &str, params: &[(&str, String)]) -> Self {
        Self {
            code: code.to_owned(),
            params: params
                .iter()
                .map(|(key, value)| ((*key).to_owned(), value.clone()))
                .collect(),
        }
    }
}

/// The card a platform shows: its layout, the other one to look at, whether the rule is unsure
/// (both are shown) and why.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub layout: Layout,
    pub alternate: Option<Layout>,
    pub unsure: bool,
    pub reason: Reason,
}

fn sure(layout: Layout, reason: Reason) -> Variant {
    Variant {
        layout,
        alternate: layout.other(),
        unsure: false,
        reason,
    }
}

fn both(reason: Reason) -> Variant {
    Variant {
        layout: Layout::Large,
        alternate: Some(Layout::Small),
        unsure: true,
        reason,
    }
}

/// The image's image facts as the reason's parameters.
fn sized(shape: Shape) -> Vec<(&'static str, String)> {
    vec![
        ("width", shape.width.to_string()),
        ("height", shape.height.to_string()),
    ]
}

/// The first bound of `size` that `shape` misses, as the reason for the small card.
fn missed(size: &Size, shape: Shape) -> Option<Reason> {
    let mut params = sized(shape);
    let narrow = size.min_width.is_some_and(|min| shape.width < min);
    let low = size.min_height.is_some_and(|min| shape.height < min);
    if narrow || low {
        return Some(match (size.min_width, size.min_height) {
            (Some(width), Some(height)) => {
                params.push(("min_width", width.to_string()));
                params.push(("min_height", height.to_string()));
                Reason::new("below", &params)
            }
            (Some(width), None) => {
                params.push(("min_width", width.to_string()));
                Reason::new("narrow", &params)
            }
            (None, _) => {
                params.push(("min_height", size.min_height.unwrap_or(0).to_string()));
                Reason::new("low", &params)
            }
        });
    }
    if let Some(max) = size.max_ratio
        && u64::from(shape.width) > u64::from(max) * u64::from(shape.height)
    {
        params.push(("max_ratio", max.to_string()));
        return Some(Reason::new("ratio", &params));
    }
    if let Some(max) = size.max_bytes
        && shape.bytes >= max
    {
        params.push(("bytes", shape.bytes.to_string()));
        params.push(("max_bytes", max.to_string()));
        return Some(Reason::new("heavy", &params));
    }
    if size.landscape && shape.width <= shape.height {
        return Some(Reason::new("not_landscape", &params));
    }
    if let Some(tolerance) = size.square_tolerance {
        // |1 - width / height| < tolerance / 100, in integers.
        let difference = u64::from(shape.width.abs_diff(shape.height)) * 100;
        if difference < u64::from(tolerance) * u64::from(shape.height) {
            return Some(Reason::new("square", &params));
        }
    }
    None
}

/// The card a platform with `rule` shows for a page whose `twitter:card` is `card`, with a share
/// image (`image` true) of `shape` (`None`: its size is not known).
#[must_use]
pub fn choose(rule: &Rule, card: Option<&str>, image: bool, shape: Option<Shape>) -> Variant {
    if !image {
        return Variant {
            layout: Layout::Text,
            alternate: None,
            unsure: false,
            reason: Reason::new("no_image", &[]),
        };
    }
    match rule {
        Rule::Card => {
            let card = card.map(|value| value.trim().to_ascii_lowercase());
            match card.as_deref() {
                Some(LARGE_CARD) => sure(Layout::Large, Reason::new("card_large", &[])),
                Some(other) if !other.is_empty() => sure(
                    Layout::Small,
                    Reason::new("card_other", &[("card", other.to_owned())]),
                ),
                _ => sure(Layout::Small, Reason::new("card_missing", &[])),
            }
        }
        Rule::Always { layout } => Variant {
            layout: *layout,
            alternate: None,
            unsure: false,
            reason: Reason::new(
                match layout {
                    Layout::Large => "always_large",
                    Layout::Small => "always_small",
                    Layout::Text => "always_text",
                },
                &[],
            ),
        },
        Rule::Unverified => both(Reason::new("unverified", &[])),
        Rule::Size(size) => match shape {
            None => both(Reason::new("size_unknown", &[])),
            Some(shape) => match missed(size, shape) {
                Some(reason) => sure(Layout::Small, reason),
                None => sure(Layout::Large, Reason::new("meets", &sized(shape))),
            },
        },
    }
}

#[cfg(test)]
#[path = "variant_tests.rs"]
mod tests;
