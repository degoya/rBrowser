//! The helpers rBrowser's crates and first-party plugins used to carry a copy of each
//! (RB-080-28, the audit of charge 12): one definition, `std` only, built for the app and for a
//! plugin's wasm32 component alike.
//!
//! - [`hex`]: lower-case hex, two digits per byte.
//! - [`lock`]: a `Mutex` lock that takes over the data of a holder that panicked.
//! - [`url`]: the origin, host and authority of a URL without a URL crate.
//! - [`json_pointer`]: the escape of one JSON pointer segment (RFC 6901).
//! - [`mask`]: the secrets of URLs and headers masked, one list of names (RB-080-37).

#![warn(unreachable_pub)]

mod hex;
pub mod json_pointer;
mod lock;
pub mod mask;
pub mod url;

pub use hex::hex;
pub use lock::lock;
