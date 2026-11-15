//! The CDP proxy's pure logic (RB-030-06, ADR 0001 Decision 4). CEF speaks CDP only over its
//! in-process pipe; the one way in from outside is rBrowser's proxy on `127.0.0.1`, which the
//! app runs (`crates/rbrowser/src/cdp_proxy/`) on top of the parts here. Nothing in this crate
//! does I/O, so all of it is tested on every platform.
//!
//! | Part | Module |
//! | --- | --- |
//! | The OS-random secrets and their constant-time comparison | [`secret`] |
//! | The proxy's token: header or path proof, never printed | [`token`] |
//! | The HTTP request head and the WebSocket upgrade | [`request`] |
//! | Which endpoint a request asks for, and its JSON answers | [`proxy`] |
//! | The targets no client sees: shell, settings window, DevTools, analyser | [`target_filter`] |
//! | Which methods a client may send (emulation is rBrowser's) | [`policy`] |
//! | `Input.*` coordinates scaled from CSS px to widget DIP | [`input`] |
//! | One client's sessions, ids and events over the shared pipe | [`session`] |

pub mod input;
pub mod policy;
pub mod proxy;
pub mod request;
pub mod secret;
pub mod session;
pub mod target_filter;
pub mod token;

pub use session::{CallError, ClientSession, FromClient, ToClient};
pub use target_filter::{HiddenTargets, TargetFilter};
pub use token::ProxyToken;
