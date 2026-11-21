//! The sitemap gallery page of the app's gallery window (RB-090-86): `getSitemapGallery` asks for
//! a run, `sitemapGallery` answers it - the run's facts, its breakpoints and per page and
//! breakpoint the shot, its image as an `rbrowser://sitemap/<run>/<slug>/<breakpoint>.png`
//! address the gallery window alone may load. TypeScript mirror: `ui/src/ipc/sitemapGallery.ts`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// `getSitemapGallery`: the run to show.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapGalleryRequest {
    /// The run's id; absent: the latest run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
}

/// A breakpoint of the run.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryBreakpoint {
    /// The file name of its shots (`mobile`).
    pub name: String,
    /// Its label; empty: the name.
    pub label: String,
    pub width: u32,
    pub height: u32,
}

/// A shot's comparison with its baseline.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryCompare {
    /// `ok`, `differs`, `new`, `sizeMismatch` or `failed`.
    pub status: String,
    /// The share of pixels that differ, 0 to 1.
    pub ratio: f64,
}

/// One page in one breakpoint.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryShot {
    /// `pending`, `done` or `failed`.
    pub state: String,
    /// Why it failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Its image on the gallery window's route; only for a shot that is done.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<GalleryCompare>,
}

/// One page of the run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GalleryPage {
    pub url: String,
    /// The HTTP status of its load.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    /// Where a redirect ended.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub final_url: Option<String>,
    /// Its shots by breakpoint name.
    pub shots: BTreeMap<String, GalleryShot>,
}

/// `sitemapGallery`: the run as the gallery page shows it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SitemapGalleryPage {
    pub run_id: String,
    /// The site or sitemap the run came from.
    pub source: String,
    /// `running`, `cancelled` or `finished`.
    pub state: String,
    /// How wide a thumbnail is, in CSS px (the setting `sitemap-capture.thumbnailWidth`).
    pub thumbnail_width: u32,
    pub breakpoints: Vec<GalleryBreakpoint>,
    /// The first pages of the run, at most the core's limit.
    pub pages: Vec<GalleryPage>,
    /// How many pages the run has, when `pages` holds fewer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_pages: Option<u32>,
}

#[cfg(test)]
#[path = "sitemap_gallery_tests.rs"]
mod tests;
