//! `visual`: Capture > Visual regression (RB-050-04) - the colour tolerance, full page or
//! viewport, how a page names its baseline folder, the Git LFS check and whether a comparison
//! against another setup fails. Re-exported from [`crate::model`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `visual`: Capture > Visual regression (RB-050-04), read by `crates/rbrowser/src/visual/`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", default)]
pub struct VisualConfig {
    /// How different two pixels may be and still count as equal, from 0 (exactly equal) to 1
    /// (anything goes); anti-aliased edges are tolerated apart from it.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub threshold: f64,
    /// Compare and store the whole page, not only the viewport.
    pub full_page: bool,
    /// How a page's URL names its folder below `.rbrowser/baselines`: its path, the host and
    /// the path, or the path and the query.
    #[serde(skip_serializing_if = "crate::shared_settings::is_default")]
    #[schemars(extend("default" = "path"))]
    pub slug_strategy: SlugStrategy,
    /// After saving a baseline, check that Git LFS tracks `.rbrowser/baselines` and is
    /// installed, and offer the set-up when not.
    #[serde(skip_serializing_if = "is_true")]
    #[schemars(extend("default" = true))]
    pub lfs_check: bool,
    /// Treat a comparison against a baseline taken with another monitor DPI, engine version,
    /// viewport or full-page setting as an error instead of a warning.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[schemars(extend("default" = false))]
    pub fail_on_meta_mismatch: bool,
}

impl Default for VisualConfig {
    fn default() -> Self {
        Self {
            threshold: 0.1,
            full_page: true,
            slug_strategy: SlugStrategy::default(),
            lfs_check: true,
            fail_on_meta_mismatch: false,
        }
    }
}

/// `visual.slugStrategy`: how a page's URL becomes its baseline folder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum SlugStrategy {
    /// The path only (`/shop/cart` is `shop-cart`): every environment shares the baselines.
    #[default]
    Path,
    /// The host, then the path: each host has its own.
    HostPath,
    /// The path, then the query: `?page=2` has its own.
    PathQuery,
}

fn is_true(value: &bool) -> bool {
    *value
}
