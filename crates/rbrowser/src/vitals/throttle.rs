//! The throttling of a view as the labels and `view_metrics` show it: read from the
//! performance budget (RB-020-09, `crate::perf`), which owns it and emits
//! `ViewEvent::Updated` on every change. The one place this module touches `crate::perf`.

use rb_engine::ViewId;
use rb_ipc::ViewThrottle;

use crate::host::Host;
use crate::perf::ThrottleState;

/// `view`'s throttling; `None` while it renders at full speed.
#[must_use]
pub fn throttle_of(host: &Host, view: ViewId) -> Option<ViewThrottle> {
    match crate::perf::throttle_state(host, view) {
        ThrottleState::Visible => None,
        ThrottleState::Hidden => Some(ViewThrottle::Hidden),
        ThrottleState::CpuLimited { fps } => Some(ViewThrottle::CpuLimited { fps }),
    }
}
