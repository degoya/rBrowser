use super::*;
use crate::fake::{SweepHost, overflow};
use crate::plan::{Span, ranges};

fn request(view: u32, from: u32, to: u32, step: u32) -> Request {
    Request {
        view,
        span: Span { from, to, step },
        kinds: vec!["overflow".to_owned()],
    }
}

fn hero_between(low: u32, high: u32) -> crate::fake::Page {
    Box::new(move |width| {
        if (low..=high).contains(&width) {
            vec![overflow(".hero img")]
        } else {
            Vec::new()
        }
    })
}

#[test]
fn a_sweep_sets_each_width_looks_at_it_and_ends_the_lease() {
    let mut host = SweepHost::with_views(&[1, 2]);
    let outcome = sweep(&mut host, &request(1, 320, 400, 10)).expect("swept");
    assert_eq!(outcome.end, End::Done);
    assert_eq!(outcome.probes, 9);
    let methods = host.methods();
    assert_eq!(methods.iter().filter(|m| *m == SET).count(), 9);
    assert_eq!(
        methods.last().map(String::as_str),
        Some(CLEAR),
        "the lease ends"
    );
    assert!(
        host.cdp.iter().all(|(view, _, _)| *view == 1),
        "the other views are never touched"
    );
    assert!(host.evaluated.iter().all(|view| *view == 1));
    assert_eq!(host.progress.last(), Some(&(9, 9)));
}

#[test]
fn a_break_between_two_steps_is_refined_to_the_pixel() {
    let mut host = SweepHost::with_views(&[1]);
    host.page = hero_between(703, 736);
    let outcome = sweep(&mut host, &request(1, 320, 1920, 10)).expect("swept");
    let found = ranges(&outcome.samples);
    assert_eq!(found.len(), 1);
    assert_eq!((found[0].from, found[0].to), (703, 736));
    // 161 coarse widths and 9 pixels on either side of the break.
    assert_eq!(outcome.probes, 161 + 18);
    let total = host.progress.last().map(|(_, total)| *total);
    assert_eq!(total, Some(179));
}

#[test]
fn escape_in_the_view_ends_the_sweep_and_gives_the_view_back() {
    let mut host = SweepHost::with_views(&[1]);
    host.escape_at = Some(500);
    let outcome = sweep(&mut host, &request(1, 320, 1920, 10)).expect("ended");
    assert_eq!(outcome.end, End::Escaped);
    assert_eq!(host.width, None, "the lease ended");
    let sets = host.methods().iter().filter(|m| *m == SET).count();
    assert_eq!(sets, 19, "320 to 500, nothing after");
    assert_eq!(host.methods().last().map(String::as_str), Some(CLEAR));
}

#[test]
fn a_cancelled_lease_ends_the_sweep_at_its_next_width() {
    let mut host = SweepHost::with_views(&[1]);
    host.cancel_after = Some(600);
    let outcome = sweep(&mut host, &request(1, 320, 1920, 10)).expect("ended");
    assert_eq!(outcome.end, End::Cancelled);
    assert_eq!(host.methods().last().map(String::as_str), Some(CLEAR));
    assert!(
        !outcome.samples.contains_key(&610),
        "no width after the cancel is looked at"
    );
}

#[test]
fn a_failing_page_still_ends_the_lease() {
    let mut host = SweepHost::with_views(&[1]);
    host.broken = true;
    let failure = sweep(&mut host, &request(1, 320, 340, 10)).expect_err("fails");
    assert_eq!(failure.code(), Some(crate::probe::PAGE_FAILED_KEY));
    assert_eq!(host.methods().last().map(String::as_str), Some(CLEAR));
}
