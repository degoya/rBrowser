//! The time of the names a person reads (RB-090-69): the screenshot and video run folders
//! (`{date}`, `{day}`, `{time}`), a recording's default name and the sitemap and
//! environment-compare run ids - local time, or UTC with `ui.fileNameTime: "utc"`. Timestamps
//! stored in JSON and metadata stay UTC (RFC 3339); the lists order the folders by when they were
//! made (`rb_visual::sitemap_store::oldest_first`), not by name.

use std::sync::atomic::{AtomicBool, Ordering};

use rb_config::FileNameTime;
use rb_visual::Stamp;

use crate::control::Core;

/// `ui.fileNameTime` is `utc`; one value for the process, set from the user settings.
static UTC: AtomicBool = AtomicBool::new(false);

/// Puts `zone` in effect for the names made from now on.
pub fn set_zone(zone: FileNameTime) {
    UTC.store(zone == FileNameTime::Utc, Ordering::Relaxed);
}

/// The zone in effect.
pub fn zone() -> FileNameTime {
    if UTC.load(Ordering::Relaxed) {
        FileNameTime::Utc
    } else {
        FileNameTime::Local
    }
}

/// Follows `ui.fileNameTime`.
pub fn on_user_settings_changed(core: &Core) {
    set_zone(core.user_settings().ui.file_name_time);
}

/// The offset east of UTC, in seconds, of `zone` at `secs` after the Unix epoch: the local
/// zone's at that instant (so daylight saving time counts; 0 when the system names no zone), 0
/// for UTC.
pub fn offset_in(zone: FileNameTime, secs: u64) -> i32 {
    match zone {
        FileNameTime::Utc => 0,
        FileNameTime::Local => i64::try_from(secs)
            .ok()
            .and_then(|secs| time::OffsetDateTime::from_unix_timestamp(secs).ok())
            .and_then(|instant| time::UtcOffset::local_offset_at(instant).ok())
            .map_or(0, time::UtcOffset::whole_seconds),
    }
}

/// [`offset_in`] the zone in effect.
pub fn offset_at(secs: u64) -> i32 {
    offset_in(zone(), secs)
}

/// The stamp of `secs` after the Unix epoch in `zone`.
pub fn stamp_in(zone: FileNameTime, secs: u64) -> Stamp {
    Stamp::from_unix_with_offset(secs, offset_in(zone, secs))
}

/// The stamp of `secs` after the Unix epoch in the zone in effect.
pub fn stamp_at(secs: u64) -> Stamp {
    stamp_in(zone(), secs)
}

/// Seconds since the Unix epoch now; 0 before it.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The stamp of now in the zone in effect.
pub fn stamp_now() -> Stamp {
    stamp_at(now_secs())
}

#[cfg(test)]
#[path = "local_time_tests.rs"]
mod tests;
