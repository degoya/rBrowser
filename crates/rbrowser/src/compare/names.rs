//! The file names of a run's images: a folder per path (`01-shop-cart`), a shot per
//! environment and breakpoint (`2-staging--1-mobile.png`), a diff per pair and breakpoint
//! (`diff-1-2--1-mobile.png`). The numbers keep two names apart that shorten alike; every name
//! is a plain name of at most 64 characters (`rb_visual::slug::valid_name`).

use rb_visual::sitemap::file_name;
use rb_visual::{SlugStrategy, slug};

/// `text` as a plain name of at most `max` characters.
fn short(text: &str, max: usize) -> String {
    let name: String = file_name(text).chars().take(max).collect();
    let name = name.trim_end_matches(['.', '-']).to_owned();
    if name.is_empty() {
        "x".to_owned()
    } else {
        name
    }
}

/// The folder of path number `index` (0-based).
pub fn path_folder(index: usize, path: &str) -> String {
    let words = slug(&format!("http://rb.invalid{path}"), SlugStrategy::PathQuery);
    format!("{:02}-{}", index + 1, short(&words, 40))
}

/// The shot of environment `environment` in breakpoint `breakpoint` (0-based), without `.png`.
pub fn shot_name(environment: usize, id: &str, breakpoint: usize, name: &str) -> String {
    format!(
        "{}-{}--{}-{}",
        environment + 1,
        short(id, 20),
        breakpoint + 1,
        short(name, 28)
    )
}

/// The diff of environments `a` and `b` in breakpoint `breakpoint` (0-based), without `.png`.
pub fn diff_name(a: usize, b: usize, breakpoint: usize, name: &str) -> String {
    format!(
        "diff-{}-{}--{}-{}",
        a + 1,
        b + 1,
        breakpoint + 1,
        short(name, 28)
    )
}

#[cfg(test)]
#[path = "names_tests.rs"]
mod tests;
