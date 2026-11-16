use super::{VIEW_SIZE, propose};

#[test]
fn one_breakpoint_per_width_narrowest_first() {
    let proposals = propose(&[(1440, 900), (375, 812), (375, 667)]);
    assert_eq!(proposals.len(), 2);
    assert_eq!(proposals[0].value, "{\"id\":\"w375\",\"width\":375,\"height\":667}");
    assert_eq!(proposals[1].params, vec![("width".to_owned(), "1440".to_owned())]);
    assert!(proposals.iter().all(|proposal| proposal.reason == VIEW_SIZE));
}

#[test]
fn no_view_no_proposal() {
    assert!(propose(&[]).is_empty());
}
