//! RB-080-47: the footer's own results, per document, beside the result of Analyze Breakpoints.

use rb_config::{FooterField, ViewFooterSettings};
use rb_engine::{ViewHost as _, ViewId};
use rb_ipc::{BoundaryKind, DetectedBreakpoint};

use super::tests::{Setup, classic, config, setup};
use crate::view_footer::PageQueries;

const START: &str = "https://a.test/start";
const OTHER: &str = "https://a.test/other";

fn min(width: u32) -> DetectedBreakpoint {
    DetectedBreakpoint {
        width,
        kind: BoundaryKind::Min,
        label: None,
        count: 1,
        source: None,
        heuristic: false,
    }
}

fn read(width: u32, skipped: usize) -> PageQueries {
    PageQueries {
        detected: vec![min(width)],
        skipped,
        capped: false,
        signature: u64::from(width),
    }
}

/// Keeps `queries` for `document` as a read that started now does.
fn keep(setup: &Setup, document: &str, queries: PageQueries) -> bool {
    let generation = setup.host.state().footer.generation;
    setup.host.footer_page_read(document, queries, generation)
}

/// Waits until the footer of `view` reads `wanted` (the footer task writes it).
async fn settled(setup: &Setup, view: ViewId, wanted: &str) {
    for _ in 0..200 {
        if text(setup, view) == wanted {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(text(setup, view), wanted);
}

fn queries_only() -> Setup {
    setup(Some(ViewFooterSettings {
        fields: [FooterField::Queries].into_iter().collect(),
        ..classic()
    }))
}

/// `view` shows `url`, and the footer task has noted it.
async fn shows(setup: &Setup, view: ViewId, url: &str) {
    setup.host.on_address_change(view, url);
    let document = url.split('#').next().unwrap_or(url);
    for _ in 0..200 {
        if setup
            .host
            .state()
            .footer
            .documents
            .get(&view)
            .map(String::as_str)
            == Some(document)
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    panic!("the document of {view} was not noted");
}

fn text(setup: &Setup, view: ViewId) -> String {
    setup.host.state().footer.sent[&view].text.clone()
}

#[tokio::test]
async fn a_read_counts_for_the_views_on_that_document_only() {
    let setup = queries_only();
    let views = setup.host.views();
    let (here, away) = (views[1].id, views[2].id);
    shows(&setup, here, "https://a.test/start#top").await;
    shows(&setup, away, OTHER).await;
    keep(&setup, START, read(600, 0));
    assert_eq!(text(&setup, here), "min-width 600px");
    // The view on another page keeps the ends of its range.
    assert_eq!(text(&setup, away), "min-width 1440px");
}

#[tokio::test]
async fn the_read_leaves_the_result_of_analyze_breakpoints_alone() {
    let setup = queries_only();
    let views = setup.host.views();
    let (read_view, other) = (views[1].id, views[2].id);
    setup
        .host
        .update_shell_state(|shell| shell.detected = vec![min(700)]);
    setup.host.footer_analysed();
    shows(&setup, read_view, START).await;
    keep(&setup, START, read(600, 0));
    // The project-wide result is what it was, and still feeds a view without a read of its own.
    assert_eq!(setup.host.shell_state().detected, vec![min(700)]);
    assert_eq!(text(&setup, read_view), "min-width 600px");
    assert_eq!(text(&setup, other), "min-width 700px");
}

#[tokio::test]
async fn a_view_that_comes_back_to_a_document_takes_its_result_again() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    keep(&setup, START, read(600, 0));
    shows(&setup, view, OTHER).await;
    assert_eq!(text(&setup, view), "min-width 768px | max-width 1439px");
    shows(&setup, view, START).await;
    for _ in 0..200 {
        if text(&setup, view) == "min-width 600px" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(text(&setup, view), "min-width 600px");
}

#[tokio::test]
async fn switching_the_analysis_off_brings_the_ends_of_the_range_back() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    keep(&setup, START, read(600, 0));
    assert_eq!(setup.host.footer_result_signature(START), Some(600));
    setup.host.footer_results_clear();
    assert_eq!(setup.host.footer_result_signature(START), None);
    assert_eq!(text(&setup, view), "min-width 768px | max-width 1439px");
}

#[tokio::test]
async fn the_tooltip_says_how_many_stylesheets_could_not_be_read() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    keep(&setup, START, read(600, 2));
    let tooltip = setup.host.state().footer.sent[&view].tooltip.clone();
    let (queries, note) = tooltip.split_once('\n').expect("two lines");
    assert_eq!(queries, "min-width 600px");
    assert!(note.contains('2') || note == "footer.skipped", "{note}");
    // The same read again writes nothing.
    let before = super::tests::footer_calls(&setup.ui).len();
    keep(&setup, START, read(600, 2));
    assert_eq!(super::tests::footer_calls(&setup.ui).len(), before);
}

#[tokio::test]
async fn an_incomplete_or_empty_read_leaves_the_queries_of_analyze_breakpoints() {
    // The owner's site with its CSS on a CDN without CORS: the document reads none of it.
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    setup
        .host
        .update_shell_state(|shell| shell.detected = vec![min(700), min(500)]);
    setup.host.footer_analysed();
    shows(&setup, view, START).await;
    for incomplete in [
        PageQueries {
            detected: Vec::new(),
            ..read(0, 0)
        },
        read(600, 3),
        PageQueries {
            capped: true,
            ..read(600, 0)
        },
    ] {
        keep(&setup, START, incomplete);
        assert_eq!(text(&setup, view), "min-width 700px");
        // What the read left out is no word in the tooltip of another result.
        let tooltip = setup.host.state().footer.sent[&view].tooltip.clone();
        assert_eq!(tooltip, "min-width 700px, min-width 500px");
    }
    // A complete read that saw less than the analysis that fetched the sheets: the analysis.
    keep(&setup, START, read(600, 0));
    assert_eq!(text(&setup, view), "min-width 700px");
}

#[tokio::test]
async fn beside_an_own_read_the_analysis_counts_for_the_pages_of_its_site() {
    let setup = queries_only();
    let mut next = config();
    next.start_url = Some("https://a.test/".to_owned());
    setup.host.open_config(next, None).expect("opens");
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    setup
        .host
        .update_shell_state(|shell| shell.detected = vec![min(700), min(500)]);
    setup.host.footer_analysed();
    // On to another page of the site: without a read of its own its queries are not known.
    shows(&setup, view, OTHER).await;
    settled(&setup, view, "min-width 768px | max-width 1439px").await;
    // An empty read of that page does not stand for it: the site's analysis does.
    keep(
        &setup,
        OTHER,
        PageQueries {
            detected: Vec::new(),
            ..read(0, 1)
        },
    );
    assert_eq!(text(&setup, view), "min-width 700px");
    // A page of another site has only what was read from it.
    shows(&setup, view, "https://b.test/").await;
    keep(&setup, "https://b.test/", read(600, 1));
    assert_eq!(text(&setup, view), "min-width 600px");
}

#[tokio::test]
async fn a_read_that_ends_after_the_switch_went_off_is_not_kept() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    let start = setup.host.footer_read_start(view).expect("start");
    assert_eq!(start.document, START);
    // The setting is switched off while the read is with the analyser.
    setup.host.footer_results_clear();
    assert!(
        !setup
            .host
            .footer_page_read(START, read(600, 0), start.generation)
    );
    assert_eq!(setup.host.footer_result_signature(START), None);
    assert_eq!(text(&setup, view), "min-width 768px | max-width 1439px");
    // A read started afterwards is.
    let again = setup.host.footer_read_start(view).expect("start");
    assert!(
        setup
            .host
            .footer_page_read(START, read(600, 0), again.generation)
    );
}

#[tokio::test]
async fn a_config_that_changes_the_analysis_drops_the_results_and_asks_for_new_reads() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    let start = setup.host.footer_read_start(view).expect("start");
    keep(&setup, START, read(600, 0));
    assert!(!setup.host.footer_take_stale());
    // A colour is the footer's own business: the results stay.
    let mut colour = config();
    colour.breakpoints[0].color = Some("#14532d".to_owned());
    setup.host.replace_config(colour);
    assert_eq!(setup.host.footer_result_signature(START), Some(600));
    assert!(!setup.host.footer_take_stale());
    // The merge tolerance changes what the analyser answers.
    let mut tolerance = config();
    tolerance.analyzer.merge_tolerance_px = 40;
    setup.host.replace_config(tolerance);
    assert_eq!(setup.host.footer_result_signature(START), None);
    assert!(setup.host.footer_take_stale());
    assert!(!setup.host.footer_take_stale(), "asked for once");
    // A read under way since before the change is not kept, and a new one carries a new mark.
    assert!(
        !setup
            .host
            .footer_page_read(START, read(600, 0), start.generation)
    );
    let again = setup.host.footer_read_start(view).expect("start");
    assert_ne!(again.config_mark, start.config_mark);
}

#[tokio::test]
async fn a_read_starts_from_the_address_the_host_knows_for_the_view() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    // No page yet: nothing to read.
    assert_eq!(setup.host.footer_read_start(view), None);
    assert_eq!(setup.host.footer_read_start(ViewId(99)), None);
    shows(&setup, view, "https://a.test/start#top").await;
    let start = setup.host.footer_read_start(view).expect("start");
    assert_eq!(start.document, START);
}

#[tokio::test]
async fn the_tooltip_says_when_a_page_has_more_media_lists_than_a_read_takes() {
    let setup = queries_only();
    let view = setup.host.views()[1].id;
    shows(&setup, view, START).await;
    keep(
        &setup,
        START,
        PageQueries {
            capped: true,
            ..read(600, 0)
        },
    );
    let tooltip = setup.host.state().footer.sent[&view].tooltip.clone();
    assert_eq!(tooltip.lines().count(), 2, "{tooltip}");
}

#[tokio::test]
async fn the_waits_and_the_switch_are_the_host_s() {
    let first = queries_only();
    let second = queries_only();
    let view = first.host.views()[0].id;
    let ticket = first.host.footer_auto(|auto| auto.due.navigated(view));
    // Another window's host knows nothing of it.
    assert!(!second.host.footer_auto(|auto| auto.due.take(view, ticket)));
    assert!(first.host.footer_auto(|auto| auto.due.take(view, ticket)));
}
