use std::collections::BTreeMap;

use super::*;

#[test]
fn without_the_window_no_header_reaches_firefox() {
    let filter = filter_for(Weak::new());
    let emulation = Emulation {
        headers: BTreeMap::from([("X-Project".to_owned(), "1".to_owned())]),
        ..Emulation::default()
    };
    let sent = filter(ViewId(3), &emulation);
    assert!(sent.headers.is_empty());
    assert_eq!(
        sent,
        Emulation {
            headers: BTreeMap::new(),
            ..emulation
        }
    );
}
