//! Marks an image in the views (the panel's hover) over CDP `DOM` and `Overlay`, the selector a
//! parameter of `DOM.querySelector`, never script text: the one `plugin_util::mark`, shared with
//! `plugins/outline` (RB-094-11).

pub use plugin_util::mark::*;

#[cfg(test)]
#[path = "mark_tests.rs"]
mod tests;
