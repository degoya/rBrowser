use super::*;

fn views() -> Vec<String> {
    ["V1", "V2", "V3"].map(str::to_owned).to_vec()
}

#[test]
fn a_slot_shows_its_view_and_its_frames_but_no_other_view() {
    let claimed = claim("V2", &views()).expect("a free slot");
    let filter = claimed.filter();
    assert!(!filter.is_hidden("V2", "https://a.test/"));
    assert!(!filter.is_hidden("IFRAME-OF-V2", "https://ads.test/"));
    assert!(filter.is_hidden("V1", "https://a.test/"));
    assert!(filter.is_hidden("V3", ""));
    // The app's own pages stay hidden as for every outside client.
    assert!(filter.is_hidden("SHELL", crate::shell::SHELL_URL));
}

#[test]
fn a_freed_slot_hides_everything_and_is_claimed_again() {
    let claimed = claim("V1", &views()).expect("a free slot");
    let filter = claimed.filter();
    drop(claimed);
    assert!(filter.is_hidden("V1", "https://a.test/"));
    let again: Vec<Claimed> = (0..SLOTS).map_while(|_| claim("V1", &views())).collect();
    assert!(!again.is_empty(), "slots come back");
    assert!(again.len() <= SLOTS);
}

#[test]
fn a_page_that_appears_during_the_run_stays_hidden() {
    use rb_cdp::target_filter::hides_info;
    use serde_json::json;

    let claimed = claim("V2", &views()).expect("a free slot");
    let filter = claimed.filter();
    let info =
        |id: &str, kind: &str| json!({ "targetId": id, "type": kind, "url": "https://a.test/" });
    // A popup of the view, or a view added while the run goes on: neither is the flow's page.
    assert!(hides_info(filter, &info("POPUP", "page")));
    assert!(hides_info(filter, &info("V4", "page")));
    assert!(hides_info(filter, &info("V1", "page")));
    // The view's own page, its frames and workers stay visible.
    assert!(!hides_info(filter, &info("V2", "page")));
    assert!(!hides_info(filter, &info("IFRAME-OF-V2", "iframe")));
    assert!(!hides_info(filter, &info("WORKER-OF-V2", "worker")));
}
