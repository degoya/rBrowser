//! The helpers rBrowser's crates and first-party plugins used to carry a copy of each
//! (RB-080-28, the audit of charge 12): one definition, `std` only, built for the app and for a
//! plugin's wasm32 component alike.
//!
//! - [`fs`]: a file written whole or not at all.
//! - [`hex`]: lower-case hex, two digits per byte.
//! - [`lock`]: a `Mutex` lock that takes over the data of a holder that panicked.
//! - [`text`]: text cut to a length in characters, with or without `...`.
//! - [`url`]: the origin, host and authority of a URL, whether it is a web page.
//! - [`json_pointer`]: the escape of one JSON pointer segment (RFC 6901).
//! - [`mask`]: the secrets of URLs and headers masked, one list of names (RB-080-37).
//! - [`win_name`]: a path segment as Windows reads it, an 8.3 short name.
//! - [`perf`]: timing counters with percentiles, kept in release builds (RB-090-32).

#![warn(unreachable_pub)]

pub mod fs;
mod hex;
pub mod json_pointer;
mod lock;
pub mod mask;
pub mod perf;
pub mod text;
pub mod url;
pub mod win_name;

pub use hex::hex;
pub use lock::lock;
