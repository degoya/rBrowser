//! The grid's breakpoints and the media query of a block's scope.
//!
//! The breakpoints are those of the views (`views.list`: each view's breakpoint id and viewport
//! width, taken from the project's breakpoint config), once each, by width. A breakpoint covers
//! the widths from its own up to one below the next wider breakpoint's; the narrowest has no lower
//! bound and the widest no upper one, so every width falls into exactly one breakpoint - as the
//! views show them, and as a stylesheet written for them reads. The sheet the views get is the
//! one a person copies: a block for chosen breakpoints is wrapped in their media query, so every
//! view applies it by its own width.

use rb_plugin_api::ViewInfo;

/// One breakpoint of the grid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breakpoint {
    pub id: String,
    pub label: String,
    /// The viewport width in CSS px.
    pub width: u32,
}

/// The grid's breakpoints, by width.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Breakpoints {
    list: Vec<Breakpoint>,
}

/// Where a block applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// In every view: no breakpoint chosen, or all of them.
    All,
    /// In the views whose width the media query (without `@media`) matches.
    Query(String),
    /// Nowhere: none of its breakpoints is in the grid.
    Nowhere,
}

impl Breakpoints {
    /// The breakpoints of `views`: the first view of each id; a view without a breakpoint id or
    /// width is left out.
    #[must_use]
    pub fn from_views(views: &[ViewInfo]) -> Self {
        let mut list: Vec<Breakpoint> = Vec::new();
        for view in views {
            if view.breakpoint.is_empty()
                || view.width == 0
                || list.iter().any(|known| known.id == view.breakpoint)
            {
                continue;
            }
            let label = if view.label.is_empty() {
                view.breakpoint.clone()
            } else {
                view.label.clone()
            };
            list.push(Breakpoint {
                id: view.breakpoint.clone(),
                label,
                width: view.width,
            });
        }
        list.sort_by(|a, b| a.width.cmp(&b.width).then_with(|| a.id.cmp(&b.id)));
        Self { list }
    }

    #[must_use]
    pub fn list(&self) -> &[Breakpoint] {
        &self.list
    }

    /// Whether `id` is a breakpoint of the grid.
    #[must_use]
    pub fn knows(&self, id: &str) -> bool {
        self.list.iter().any(|known| known.id == id)
    }

    /// The scope of a block for the breakpoints `selected`: adjacent ranges joined, the others a
    /// list; a breakpoint the grid does not show is passed over.
    #[must_use]
    pub fn scope(&self, selected: &[String]) -> Scope {
        if selected.is_empty() {
            return Scope::All;
        }
        let mut widths: Vec<u32> = self.list.iter().map(|known| known.width).collect();
        widths.dedup();
        let chosen: Vec<bool> = widths
            .iter()
            .map(|width| {
                self.list
                    .iter()
                    .any(|known| known.width == *width && selected.contains(&known.id))
            })
            .collect();
        if !chosen.contains(&true) {
            return Scope::Nowhere;
        }
        if chosen.iter().all(|is| *is) {
            return Scope::All;
        }
        let mut parts = Vec::new();
        let mut index = 0;
        while index < widths.len() {
            if !chosen.get(index).copied().unwrap_or(false) {
                index += 1;
                continue;
            }
            let start = index;
            while chosen.get(index + 1).copied().unwrap_or(false) {
                index += 1;
            }
            let min = if start == 0 {
                None
            } else {
                widths.get(start).copied()
            };
            // Widths are distinct and above 0, so the next one is at least 2.
            let max = widths.get(index + 1).map(|next| next - 1);
            parts.push(condition(min, max));
            index += 1;
        }
        Scope::Query(parts.join(", "))
    }
}

fn condition(min: Option<u32>, max: Option<u32>) -> String {
    match (min, max) {
        (Some(min), Some(max)) => format!("(min-width: {min}px) and (max-width: {max}px)"),
        (Some(min), None) => format!("(min-width: {min}px)"),
        (None, Some(max)) => format!("(max-width: {max}px)"),
        (None, None) => "all".to_owned(),
    }
}

/// `css` inside `@media <query> { ... }`, each rule's line indented by two spaces.
#[must_use]
pub fn wrap(query: &str, css: &str) -> String {
    let body: Vec<String> = css
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("  {line}")
            }
        })
        .collect();
    format!("@media {query} {{\n{}\n}}", body.join("\n"))
}

#[cfg(test)]
#[path = "media_tests.rs"]
mod tests;
