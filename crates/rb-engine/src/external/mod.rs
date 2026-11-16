//! The external windows (RB-040-01): Firefox and WebKit views live in windows of their own,
//! synced like the grid. [`placement`] says where a window goes, [`registry`] holds the views
//! and their lifecycle without a runtime, [`ExternalWindows`] runs it on tokio: start on
//! demand, crash and close detection, restart with backoff, arrange and raise, close with the
//! app. [`embed`] is the pure half of showing such a window in its grid cell (RB-080-39),
//! [`scale`] of scaling its page to the cell (RB-080-40).

pub mod embed;
pub mod manager;
pub mod placement;
pub mod registry;
pub mod scale;

pub use embed::{
    Candidate, EmbedPlace, NativeWindow, WindowStyle, content_inset, embedded_style, pick_window,
    place_in_cell,
};
pub use manager::{ExternalWindows, OnChange, RestartError, StartReason, not_available_yet};
pub use placement::{CASCADE_STEP, GAP, Placement, PlacementError, Screen, WindowRect, place};
pub use registry::{
    Backoff, ExitAction, ExternalSpec, ExternalStatus, ExternalView, Registry, RegistryError,
    STABLE_RUN,
};
pub use scale::{CellPixels, CellScale, cell_factor, cell_scale, quantize, shared_factor};
