//! What the agent of the view a recording records from does for it (RB-090-87): while the
//! recording takes steps, that view's `AgentConfig.record` carries `automation.recorder.*`'s
//! wishes - report a rest of `hoverDwellMs` on a menu's entry (unless `recordHover` is `off`) and
//! send the pointer's way as `recordPointer` says. The SyncHub puts it into the configuration of
//! the view that captures ([`crate::sync_hub`]) and of the Firefox agents
//! (`engines::firefox::live`); a start, pause, resume or stop reconfigures the agents. Every
//! other view, and every view while no recording takes steps, has none. The wishes are held
//! apart from the recorder, so reading them never waits for a recording being saved. WebKit
//! views get none: their agents are configured only when the settings change.

use rb_config::{RecordHover, RecordPointer, RecorderConfig};
use rb_sync::{AgentRecord, AgentRecordPointer};

use super::glue::HostRecorder;
use crate::cdp::lock;
use crate::host::Host;

/// The wishes of `config` for the recorded view's agent.
pub fn wishes(config: &RecorderConfig) -> AgentRecord {
    AgentRecord {
        menu_hovers: config.record_hover != RecordHover::Off,
        hover_dwell_ms: u32::try_from(config.hover_dwell_ms).unwrap_or(u32::MAX),
        record_pointer: match config.record_pointer {
            RecordPointer::Menu => AgentRecordPointer::Menu,
            RecordPointer::All => AgentRecordPointer::All,
            RecordPointer::Off => AgentRecordPointer::Off,
        },
    }
}

impl HostRecorder {
    /// The recorded view's wishes while the recording takes steps; none while paused or idle.
    pub fn agent_record(&self) -> Option<AgentRecord> {
        *lock(&self.wishes)
    }

    /// The recording started or resumed (`Some`), paused or stopped (`None`): the agents follow.
    /// Called with the recorder's lock released.
    pub(super) fn follow(&self, host: &Host, wishes: Option<AgentRecord>) {
        *lock(&self.wishes) = wishes;
        if let Some(sync) = crate::sync_hub::for_host(host) {
            sync.recorder_changed();
        }
    }
}

/// The wishes for the view that captures in `host`'s window: none without a recording.
pub fn agent_record(host: &Host) -> Option<AgentRecord> {
    super::for_host(host).and_then(|recorder| recorder.agent_record())
}

#[cfg(test)]
#[path = "agent_record_tests.rs"]
mod tests;
