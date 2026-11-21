//! Engine comparison (RB-094-03): one page at one breakpoint in Chromium, Firefox and WebKit,
//! the same size and DPR, each engine pair diffed pixel by pixel with the colour tolerance and
//! the anti-aliasing test of `rb-visual` (RB-050-04), so font smoothing is no difference and a
//! layout that differs is. Built on the environment comparison (RB-080-03): its record, sink and
//! report page, with the engines as the environments.
//!
//! - [`plan`]: the engines (one that cannot run is reported, not an error), the breakpoint's
//!   shot spec, the page and `engineCompare.*` held to its bounds.
//! - [`backend`]: Firefox's and WebKit's shots as a render backend, in a window of their own.
//! - [`run`]: one comparison at a time, every engine at once, `report.json` and `report.html`
//!   ([`report`]).
//! - [`app`]: the command ([`defs`]: `compare.engines`, `Leader C Q`) and the MCP tool's entry
//!   point ([`tool`]: `compare_engines`).
//!
//! Output: `.rbrowser/out/engine-compare/<run>/page/<n>-<engine>--1-<breakpoint>.png`, the diff
//! images beside them, `report.json`, `report.html`.

pub mod app;
pub mod backend;
pub mod defs;
pub mod plan;
pub mod report;
pub mod run;
pub mod tool;

pub use defs::register;
