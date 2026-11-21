use super::*;

#[test]
fn a_session_of_no_view_is_tried_once_until_the_views_change() {
    let mut sessions = Sessions::default();
    assert!(sessions.try_enable("DEVTOOLS"), "the first event tries");
    assert!(!sessions.try_enable("DEVTOOLS"), "later events do not");
    sessions.views_changed();
    assert!(sessions.try_enable("DEVTOOLS"), "a view event tries again");
}

#[test]
fn a_view_s_session_is_found_and_a_detached_one_is_forgotten() {
    let mut sessions = Sessions::default();
    sessions.enabled.insert(ViewId(1), "S1".to_owned());
    assert_eq!(sessions.view_of("S1"), Some(ViewId(1)));
    assert_eq!(sessions.view_of("S2"), None);
    assert!(sessions.try_enable("S2"));
    sessions.detached("S2");
    assert!(
        sessions.try_enable("S2"),
        "a detached session is not remembered"
    );
    sessions.closed(ViewId(1));
    assert_eq!(sessions.view_of("S1"), None);
}

fn current() -> Vec<(ViewId, String)> {
    vec![(ViewId(1), "S1".to_owned()), (ViewId(2), "S2".to_owned())]
}

fn network_calls(calls: &[(ViewId, String, &str)]) -> usize {
    calls
        .iter()
        .filter(|(_, _, method)| *method == rb_mcp::logs::NETWORK_DOMAIN)
        .count()
}

/// RB-090-34: `Network` is enabled only once the requests are wanted, then on every view once.
#[test]
fn network_is_enabled_only_once_wanted_and_then_once_per_session() {
    let mut sessions = Sessions::default();
    let calls = sessions.plan(&current(), false);
    assert_eq!(calls.len(), 2 * rb_mcp::logs::DOMAINS.len());
    assert_eq!(network_calls(&calls), 0, "not wanted: not enabled");
    assert!(sessions.plan(&current(), false).is_empty(), "nothing new");
    let calls = sessions.plan(&current(), true);
    assert_eq!(calls.len(), 2, "only Network, on both views");
    assert_eq!(network_calls(&calls), 2);
    assert!(sessions.plan(&current(), true).is_empty(), "once");
    // A reattached view gets every domain again, Network included.
    let moved = vec![(ViewId(1), "S9".to_owned()), (ViewId(2), "S2".to_owned())];
    let calls = sessions.plan(&moved, true);
    assert_eq!(calls.len(), rb_mcp::logs::DOMAINS.len() + 1);
    assert!(
        calls
            .iter()
            .all(|(view, session, _)| *view == ViewId(1) && session == "S9")
    );
    sessions.closed(ViewId(1));
    assert_eq!(sessions.view_of("S9"), None);
}
