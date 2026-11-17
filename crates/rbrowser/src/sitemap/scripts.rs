//! The page scripts of a render view (`Runtime.evaluate`): whether the page has loaded, its
//! navigation's status and address, whether a selector matches and scrolling for lazy content
//! (banners are hidden by the capture preparation, `crate::capture::clean`). A selector reaches a script only as a JSON string literal, never
//! as code; a selector the page cannot parse is skipped, not an error.

/// `document.readyState`.
pub const READY_STATE: &str = "document.readyState";

/// The main document's HTTP status (`0` when unknown) and the address it ended at.
pub const NAVIGATION: &str = "(() => { const entry = performance.getEntriesByType('navigation')[0]; \
return { status: (entry && entry.responseStatus) || 0, url: location.href }; })()";

/// The most scroll steps (one viewport each) before the page counts as scrolled.
pub const MAX_SCROLL_STEPS: u32 = 40;

/// Scrolls the page down a viewport at a time (at most [`MAX_SCROLL_STEPS`], a pause after
/// each so lazy content loads), then back to the top.
pub const AUTO_SCROLL: &str = "(async () => { const pause = () => new Promise((done) => \
setTimeout(done, 150)); const root = document.scrollingElement || document.documentElement; \
for (let step = 0; step < 40; step += 1) { const before = root.scrollTop; \
window.scrollBy(0, window.innerHeight); await pause(); \
if (root.scrollTop === before) { break; } } window.scrollTo(0, 0); await pause(); \
return true; })()";

/// `text` as a JavaScript string literal.
fn literal(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

/// Whether an element matches `selector` (`false` for a selector the page cannot parse).
pub fn has_selector(selector: &str) -> String {
    format!(
        "(() => {{ try {{ return document.querySelector({}) !== null; }} \
catch (error) {{ return false; }} }})()",
        literal(selector)
    )
}

#[cfg(test)]
#[path = "scripts_tests.rs"]
mod tests;
