//! UDIDs (RB-095-04): what a simulator's or a device's id looks like before it goes into a
//! command line (`xcrun simctl boot <udid>`) or a capability, a device's from its USB serial
//! number, and the short form logs carry - a device's UDID identifies the device, so no log line,
//! error or diagnostics ZIP holds it whole.

/// The longest UDID taken.
pub const MAX_LEN: usize = 64;

/// Whether `text` is shaped like a UDID: 20 to 64 hex digits and dashes, starting with a digit
/// or letter (never an option of a command line).
pub fn is_udid(text: &str) -> bool {
    let digits = text.chars().filter(char::is_ascii_hexdigit).count();
    (20..=MAX_LEN).contains(&text.len())
        && text.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
        && text.chars().next().is_some_and(|c| c.is_ascii_hexdigit())
        && digits >= 20
}

/// The UDID of an iPhone or iPad from the serial number its USB port reports: 24 hex digits get
/// the dash after the eighth (devices since 2018), 40 stay as they are; `None` for anything else.
pub fn from_usb_serial(serial: &str) -> Option<String> {
    let serial = serial.trim();
    if is_udid(serial) && serial.contains('-') {
        return Some(serial.to_owned());
    }
    if !serial.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    match serial.len() {
        24 => Some(format!("{}-{}", &serial[..8], &serial[8..])),
        40 => Some(serial.to_owned()),
        _ => None,
    }
}

/// `udid` for a log line: its first and last four characters.
pub fn short(udid: &str) -> String {
    if udid.len() < 12 || !udid.is_ascii() {
        return "...".to_owned();
    }
    format!("{}...{}", &udid[..4], &udid[udid.len() - 4..])
}

#[cfg(test)]
#[path = "udid_tests.rs"]
mod tests;
