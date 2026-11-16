//! Environment comparison (RB-080-03): the same page in two or more environments (local,
//! staging, prod), side by side and as a pixel-diff run. Built from what is there: the
//! project's environments (RB-010-12), navigation sync's environment-aware follower URL
//! (RB-020-06), the sitemap capture's hidden render views and batch (RB-050-05) and the pixel
//! diff of `rb-visual` (RB-050-04).
//!
//! - [`plan`]: environments, paths, breakpoints, pairs and `envCompare.*` held to its bounds.
//! - [`live`]: `env.compare.live` (`Leader E C`), the active breakpoint once per environment.
//! - [`runs`]: one run at a time - the shots in hidden render views, each breakpoint and path
//!   diffed across every environment pair on a blocking thread as soon as its shots are in,
//!   `report.json` ([`report`]) and `report.html` ([`html`]); file names in [`names`]; the
//!   shots' sink in `sink`.
//! - [`app`]: the commands ([`defs`]: `env.compare.live`, `env.compare.run` `Leader E R`,
//!   `env.compare.report` `Leader E O`, `env.compare.cancel` `Leader E X`) and the MCP tools'
//!   entry points ([`tool`]).
//!
//! Output: `.rbrowser/out/env-compare/<run>/<nn-path>/<env>--<breakpoint>.png`, the diff
//! images beside them, `report.json`, `report.html`.

pub mod app;
pub mod defs;
pub mod html;
pub mod live;
pub mod names;
pub mod plan;
pub mod report;
pub mod runs;
mod sink;
pub mod tool;

pub use defs::register;
