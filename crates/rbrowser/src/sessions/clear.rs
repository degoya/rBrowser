//! "Clear session data": what is cleared. Every cookie of the session's request context (the
//! window does that through CEF), and per origin its views show, the origin's storage over CDP
//! (`Storage.clearDataForOrigin`, `storageTypes: "all"`: local and session storage, IndexedDB,
//! cache storage, service workers). Storage of origins no view of the session shows stays; the
//! next "clear" while such a page is open removes it.

use rb_util::url::origin_of;

/// The `storageTypes` of `Storage.clearDataForOrigin`.
pub const STORAGE_TYPES: &str = "all";

/// The origins of `urls` to clear, each once, in order.
pub fn origins_to_clear<'a>(urls: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut origins: Vec<String> = Vec::new();
    for origin in urls.into_iter().filter_map(origin_of) {
        if !origins.contains(&origin) {
            origins.push(origin);
        }
    }
    origins
}

#[cfg(test)]
#[path = "clear_tests.rs"]
mod tests;
