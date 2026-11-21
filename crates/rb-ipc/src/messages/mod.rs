//! The messages between the shell UI and the core. JSON: an object tagged `{"type": ...}`,
//! variant names and fields in camelCase. Mirrored by hand in `ui/src/ipc/protocol.ts`; the
//! JSON shape is pinned by `lib_tests.rs` and, against the TS mirror, by
//! `tests/contract_fixtures.rs` over `ui/src/ipc/contract-fixtures.json`. The payloads of the
//! messages are in `payloads.rs`; the messages of wave 0.1-w1b (`docs/contracts-0.1-w1b.md`)
//! name their job.
//!
//! One file per direction, so a wave can add variants side by side: the shell's requests
//! (`shell_to_core.rs`), the core's answers and events (`core_to_shell.rs`) and the state they
//! carry (`state.rs`); everything is re-exported here, so `crate::messages::*` paths hold. The
//! payloads of the Videos panel's requests are `video_requests.rs` (RB-093-04 made room).

mod core_to_shell;
mod shell_to_core;
mod state;
mod sync_feature;
mod video_requests;

pub use core_to_shell::CoreToShell;
pub use shell_to_core::{ShellToCore, SyncFeature};
pub use state::{LayoutState, ProjectInfo, SettingValue, StateSnapshot, SyncState};
pub use video_requests::{ConvertVideoRequest, VideoFileRef};
