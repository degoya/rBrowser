//! The secrets of the app's own endpoints - the one-time DevTools token (RB-010-06), the
//! instance nonce (RB-010-11) and the CDP proxy's token (RB-030-06): one random source and one
//! comparison, kept in `rb-cdp` ([`rb_cdp::secret`]) and used from here under their old path.

pub use rb_cdp::secret::{constant_time_eq, hex, random_hex};
