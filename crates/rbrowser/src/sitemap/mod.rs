//! Sitemap capture (RB-050-05): every page of a site's sitemap in every breakpoint, in hidden
//! render views, while the grid stays usable. The plugin `sitemap-capture` finds and picks the
//! pages and renders the gallery; this module does what needs the browser or the disk:
//!
//! - [`plan`]: the call and the project config become the plugin's source and rewrite and the
//!   breakpoints' render specs (`sitemap.*` held to its bounds by [`settings`]).
//! - [`runs`]: one run at a time - discover, `run.json`, the batch of `rb_engine::chromium` with
//!   parallelism, per-host throttle, retries and cancel, resume without a page taken twice, the
//!   gallery - against traits, so it is tested without a browser ([`status`] answers it).
//! - [`backend`]: the render views as hidden targets of the app's CDP connection, with each
//!   breakpoint's emulation, the wait conditions ([`scripts`]) and the screenshot.
//! - [`sink`]: the PNGs, the baseline comparison and `run.json`, off the async workers.
//! - [`app`]: the commands ([`defs`]: `sitemap.capture` `Leader C M`, `sitemap.cancel`
//!   `Leader C X`, `panels.sitemap` `Leader P G`), the MCP tools' entry points ([`tool`]) and the
//!   shell's progress; [`http`] is the plugins' HTTP transport.
//!
//! Output: `.rbrowser/out/sitemap-capture/<run>/<page>/<breakpoint>.png`, `run.json`,
//! `index.html`.

pub mod app;
pub mod backend;
pub mod defs;
pub mod http;
mod job;
pub mod plan;
pub mod runs;
pub mod scripts;
pub mod settings;
pub mod sink;
pub mod status;
pub mod tool;

pub use defs::register;
