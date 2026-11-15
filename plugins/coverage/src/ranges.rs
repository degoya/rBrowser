//! Range arithmetic: CSS rule usage and V8 block coverage turned into used segments of a file.
//!
//! Offsets are what CDP reports: UTF-16 code units from the start of the style sheet or script
//! (DevTools' coverage counts the same units and calls them bytes). A segment is `(start, end)`,
//! end exclusive.

/// A segment of a file, `(start, end)`, end exclusive.
pub type Segment = (u32, u32);

/// One V8 coverage range with its execution count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Count {
    pub start: u32,
    pub end: u32,
    pub count: u64,
}

impl Count {
    #[must_use]
    pub const fn new(start: u32, end: u32, count: u64) -> Self {
        Self { start, end, count }
    }
}

/// `segments` sorted, empty ones dropped, overlapping and touching ones joined.
#[must_use]
pub fn merge(mut segments: Vec<Segment>) -> Vec<Segment> {
    segments.retain(|(start, end)| start < end);
    segments.sort_unstable();
    let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
    for (start, end) in segments {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// The units `segments` (merged) cover.
#[must_use]
pub fn length(segments: &[Segment]) -> u64 {
    segments
        .iter()
        .map(|(start, end)| u64::from(end.saturating_sub(*start)))
        .sum()
}

/// The used segments of a script from V8's block coverage: ranges nest, and at every offset the
/// innermost range containing it decides (DevTools' rule); the used ones are those with a count
/// above zero, merged.
#[must_use]
pub fn used_segments(ranges: &[Count]) -> Vec<Segment> {
    let mut sorted: Vec<Count> = ranges
        .iter()
        .copied()
        .filter(|range| range.start < range.end)
        .collect();
    // Outer before inner: by start, then the longer first.
    sorted.sort_unstable_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    let mut points: Vec<u32> = sorted.iter().flat_map(|r| [r.start, r.end]).collect();
    points.sort_unstable();
    points.dedup();
    let mut used = Vec::new();
    let mut stack: Vec<Count> = Vec::new();
    let mut next = 0;
    for pair in points.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        while stack.last().is_some_and(|top| top.end <= from) {
            stack.pop();
        }
        while let Some(range) = sorted.get(next).filter(|range| range.start <= from) {
            if range.end > from {
                // Pop what ends before this range does not contain it: ranges nest.
                while stack.last().is_some_and(|top| top.end < range.end) {
                    stack.pop();
                }
                stack.push(*range);
            }
            next += 1;
        }
        if stack.last().is_some_and(|top| top.count > 0) {
            used.push((from, to));
        }
    }
    merge(used)
}

/// Whether one segment of `used` (merged) lies over all of `start..end`.
#[must_use]
pub fn covered(used: &[Segment], start: u32, end: u32) -> bool {
    used.iter().any(|(from, to)| *from <= start && end <= *to)
}

/// Whether a segment of `used` shares a unit with `start..end`.
#[must_use]
pub fn overlaps(used: &[Segment], start: u32, end: u32) -> bool {
    used.iter().any(|(from, to)| *from < end && start < *to)
}

#[cfg(test)]
#[path = "ranges_tests.rs"]
mod tests;
