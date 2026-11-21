use std::sync::Arc;

use rb_engine::ViewId;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::host::Host;
use crate::host::ui::recording::RecordingUi;

/// RB-094-16: what is forced reaches the shell's state, the activity bar's entry; the answer
/// says whether an entry was shown before, so the last state gone can be told once.
#[tokio::test(flavor = "multi_thread")]
async fn the_forced_states_reach_the_shell_and_say_whether_some_were_shown() {
    let (_fake, router) = FakeBrowser::connect();
    let host = Host::new(Arc::new(RecordingUi::default()), router, Handle::current());
    let mut registry = Registry::default();
    registry.open(ViewId(1), "F1".to_owned(), 1);
    registry.apply(
        ViewId(1),
        10,
        vec![10, 9],
        [PseudoClass::Hover].into_iter().collect(),
    );
    assert!(
        !show(&host, registry.activity()),
        "nothing was shown before"
    );
    assert_eq!(host.shell_state().forced_states, registry.activity());
    assert_eq!(host.shell_state().forced_states[0].states, ["hover"]);
    assert!(show(&host, Vec::new()), "the entry was shown");
    assert!(host.shell_state().forced_states.is_empty());
    assert!(!show(&host, Vec::new()), "and is gone");
}
