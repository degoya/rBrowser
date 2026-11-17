use super::{View, count_views};

#[test]
fn counts_the_views_and_sorts_their_widths() {
    let views = [
        View {
            id: 2,
            width: 1440,
        },
        View { id: 1, width: 375 },
    ];
    assert_eq!(count_views(&views), "{\"count\":2,\"widths\":[375,1440]}");
}

#[test]
fn no_view_is_a_count_of_zero() {
    assert_eq!(count_views(&[]), "{\"count\":0,\"widths\":[]}");
}
