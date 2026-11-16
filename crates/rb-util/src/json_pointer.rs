//! JSON pointers (RFC 6901).

/// One pointer segment escaped: `~` as `~0`, then `/` as `~1`, so a key with either reads back
/// as itself.
#[must_use]
pub fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

#[cfg(test)]
#[path = "json_pointer_tests.rs"]
mod tests;
