//! A view popped out of the grid into a window of its own at its real size (RB-080-14): zoom 1,
//! CSS px = DIP of the emulated width, still synced; closing the window puts it back into its
//! cell. The view keeps its id, browser, session and emulation: the window side moves the one
//! browser view between the grid and the window (CEF Views re-parents a view between widgets
//! without reloading it, as the overlays of the layout modes do), never a second browser.
//!
//! - [`bounds`]: where the window goes and how large it is, the title (pure).
//! - [`store`]: each breakpoint's last window bounds (`pop-out.json`).
//! - [`commands`]: `view.pop_out` and `view.pop_in`, their MCP tools `view_pop_out` and
//!   `view_pop_in`.
//! - `crate::host` keeps which views are out; `cef_app/pop_out.rs` is the window.

pub mod bounds;
pub mod commands;
pub mod store;

pub use bounds::{ScreenRect, metrics};
pub use commands::{POP_IN, POP_OUT, register, window_closed};
