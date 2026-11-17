//! Screenshots without a browser (RB-050-03): where the files go and what they are called
//! ([`naming`], [`settings`], [`store`]), the delay steps ([`delay`]), and the overview image of
//! every view with its label and size ([`overview`]), composed from the PNGs the views gave
//! ([`raster`]). The capture itself runs over each view's CDP session in the app
//! (`crates/rbrowser/src/capture/`); this crate never touches a browser.
//!
//! Visual regression (RB-050-04): the pixel diff with its anti-aliasing test and the boxes of
//! what differs ([`diff`], [`regions`]), the baselines of a project below `.rbrowser/baselines`
//! with their metadata ([`baseline`], [`meta`], [`slug`]), the comparison of one view
//! ([`compare`]) and the Git LFS check ([`lfs`]).
//!
//! The capture preparation (RB-080-12): the noise selectors, frozen animations and the page
//! script that does and undoes both ([`prepare`]).
//!
//! Sitemap capture (RB-050-05): the state of a run, which resumes without taking a shot twice
//! ([`sitemap`]), and its folder below `.rbrowser/out/sitemap-capture` ([`sitemap_store`]).

pub mod baseline;
pub mod compare;
pub mod delay;
pub mod diff;
mod font;
pub mod lfs;
pub mod meta;
pub mod naming;
pub mod overview;
pub mod prepare;
pub mod raster;
pub mod regions;
pub mod settings;
pub mod sitemap;
pub mod sitemap_store;
pub mod slug;
pub mod store;

pub use baseline::{Baseline, BaselineStore};
pub use compare::{Compared, Comparison, Status, Subject, compare};
pub use delay::Delay;
pub use diff::{DiffOptions, DiffOutcome, PixelDiff, Size, diff};
pub use meta::{BaselineMeta, MetaMismatch, Viewport};
pub use naming::{PathVars, Stamp, expand, size_text};
pub use overview::{OverviewOptions, Tile, compose};
pub use prepare::{CleanOverrides, CleanPlan, CleanSettings};
pub use raster::{Rgba, decode_png, encode_png, png_size};
pub use settings::CaptureSettings;
pub use slug::{SlugStrategy, slug};
pub use store::save_png;

/// What can go wrong between a captured PNG and the file on disk.
#[derive(Debug, thiserror::Error)]
pub enum VisualError {
    /// `capture.pathPattern` is not usable.
    #[error("invalid path pattern: {0}")]
    Pattern(String),
    /// An overview of no views.
    #[error("there is no view to capture")]
    Empty,
    /// `capture.dir` is not usable.
    #[error("invalid screenshot folder: {0}")]
    Folder(String),
    /// A PNG could not be read or written.
    #[error("PNG: {0}")]
    Png(String),
    /// An image would be larger than [`raster::MAX_PIXELS`] or a side larger than
    /// [`raster::MAX_SIDE`].
    #[error("the image would be too large ({width} x {height})")]
    TooLarge {
        /// The width asked for.
        width: u64,
        /// The height asked for.
        height: u64,
    },
    /// A baseline path that would leave `.rbrowser/baselines`, or a link on the way.
    #[error("{0} is not a plain file or folder inside the project")]
    Outside(String),
    /// Git LFS is missing or its set-up failed.
    #[error("Git LFS: {0}")]
    Lfs(String),
    /// Writing the file failed.
    #[error("writing {path}: {source}")]
    Io {
        /// The file.
        path: String,
        /// The cause.
        source: std::io::Error,
    },
}
