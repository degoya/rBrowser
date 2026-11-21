//! The forced pseudo-class states in the snapshot (RB-094-16): every view with a state forced
//! on one of its elements (`view.force_state.set`, RB-090-53) and the states, so the activity
//! bar shows them with Reset (`forcedStates`; mirror: `ui/src/ipc/activity.ts`). Absent while
//! no view has one.

use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

/// One view's forced states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewForcedStates {
    pub view: ViewId,
    /// CDP's words for the states forced on any of its elements (`hover`, `focus-within`), in
    /// the menu's order, each once.
    pub states: Vec<String>,
}

#[cfg(test)]
#[path = "forced_states_tests.rs"]
mod tests;
