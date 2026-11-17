//! An rBrowser analyzer plugin: proposes one breakpoint for every distinct view width of the grid.
//!
//! What the host promises an analyzer (docs/plugins.md):
//!
//! - `analyze` answers proposals; a person applies them, a plugin never writes the project
//!   config itself.
//! - A proposal names a JSON Pointer into `.rbrowser/config.json` (`/breakpoints/-` appends), its
//!   value as JSON text and its reason as one of the plugin's i18n keys - anything else is
//!   refused. At most 1 000 proposals.
//!
//! The logic lives here and is tested natively (`cargo test`); `src/guest.rs` only translates it
//! into the vocabulary of `wit/rbrowser-plugin.wit` and exists on `wasm32` alone.

#[cfg(target_arch = "wasm32")]
mod guest;

/// The proposal's i18n key.
pub const VIEW_SIZE: &str = "plugin.example-analyzer.reasons.view_size";

/// One proposal as the logic makes it.
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub pointer: String,
    pub value: String,
    pub reason: String,
    pub params: Vec<(String, String)>,
}

/// One breakpoint appended per distinct `(width, height)`, narrowest first.
#[must_use]
pub fn propose(sizes: &[(u32, u32)]) -> Vec<Proposal> {
    let mut sizes = sizes.to_vec();
    sizes.sort_unstable();
    sizes.dedup_by_key(|(width, _)| *width);
    sizes
        .into_iter()
        .map(|(width, height)| Proposal {
            pointer: "/breakpoints/-".to_owned(),
            value: format!("{{\"id\":\"w{width}\",\"width\":{width},\"height\":{height}}}"),
            reason: VIEW_SIZE.to_owned(),
            params: vec![("width".to_owned(), width.to_string())],
        })
        .collect()
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
