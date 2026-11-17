//! The Lighthouse bar below the vitals bar (RB-090-73): a line of the same strip for a view whose
//! page and breakpoint have a Lighthouse run, its height taken from the view like the other
//! bars', switched on its own (`ui.viewFooter.lighthouse`).

use rb_config::ViewFooterSettings;
use rb_engine::{ViewHost as _, ViewId};

use super::super::super::Host;
use super::super::tests::{area, classic, setup};
use crate::view_footer::lighthouse::{Category, Gauge, Measured};
use crate::view_footer::{CellFooter, FOOTER_HEIGHT};

const PAGE: &str = "https://example.test/shop";

fn sent(host: &Host, view: ViewId) -> Option<CellFooter> {
    host.state().footer.sent.get(&view).cloned()
}

/// The view showing breakpoint `id`.
fn view_of(host: &Host, id: &str) -> ViewId {
    host.views()
        .iter()
        .find(|view| view.breakpoint.as_deref() == Some(id))
        .map(|view| view.id)
        .expect("a view of the breakpoint")
}

fn run(id: &str, breakpoint: &str, page: &str, score: u8) -> Measured {
    Measured {
        run: id.to_owned(),
        started_at: Some(1_794_925_920),
        breakpoint: breakpoint.to_owned(),
        documents: vec![page.to_owned()],
        gauges: Category::ALL
            .iter()
            .map(|category| Gauge {
                category: *category,
                score: Some(score),
            })
            .collect(),
    }
}

#[tokio::test]
async fn a_view_whose_page_and_breakpoint_have_a_run_shows_its_scores_below_the_breakpoint_bar() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let xs = view_of(&setup.host, "xs");
    let md = view_of(&setup.host, "md");
    for view in [xs, md] {
        setup.host.footer_navigated(view, &format!("{PAGE}#top"));
    }
    let before = setup.host.footer_height(md);
    setup.host.set_footer_lighthouse(vec![
        run("20261117-100000", "xs", PAGE, 40),
        run("20261117-120000", "xs", PAGE, 96),
    ]);
    let footer = sent(&setup.host, xs).expect("a footer");
    let bar = footer.lighthouse.as_ref().expect("the Lighthouse bar");
    assert_eq!(bar.run, "20261117-120000", "the newest run");
    assert_eq!(bar.gauges.len(), 4);
    assert!(footer.lighthouse_shown);
    assert_eq!(footer.lines.len(), 1, "the breakpoint bar keeps its line");
    assert_eq!(setup.host.footer_height(xs), 2 * FOOTER_HEIGHT);
    assert_eq!(
        setup.host.footer_lighthouse_target(xs),
        Some(("20261117-120000".to_owned(), "xs".to_owned()))
    );
    // No run of md: no bar, its footer keeps its height.
    let other = sent(&setup.host, md).expect("the breakpoint bar");
    assert!(other.lighthouse.is_none());
    assert_eq!(setup.host.footer_height(md), before);
    assert_eq!(setup.host.footer_lighthouse_target(md), None);
}

#[tokio::test]
async fn another_page_has_no_bar_until_it_was_measured() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let xs = view_of(&setup.host, "xs");
    setup.host.footer_navigated(xs, PAGE);
    setup
        .host
        .set_footer_lighthouse(vec![run("20261117-100000", "xs", PAGE, 72)]);
    assert!(sent(&setup.host, xs).and_then(|f| f.lighthouse).is_some());
    setup.host.footer_navigated(xs, "https://example.test/cart");
    let footer = sent(&setup.host, xs).expect("the breakpoint bar");
    assert!(footer.lighthouse.is_none(), "a run of another page");
    assert_eq!(setup.host.footer_height(xs), FOOTER_HEIGHT);
    // Back on the measured page, the bar is back.
    setup.host.footer_navigated(xs, PAGE);
    assert!(sent(&setup.host, xs).and_then(|f| f.lighthouse).is_some());
}

#[tokio::test]
async fn off_the_lighthouse_bar_gives_its_line_back_and_shows_alone_without_the_others() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let xs = view_of(&setup.host, "xs");
    setup.host.footer_navigated(xs, PAGE);
    setup
        .host
        .set_footer_lighthouse(vec![run("20261117-100000", "xs", PAGE, 72)]);
    assert_eq!(setup.host.footer_height(xs), 2 * FOOTER_HEIGHT);
    setup.host.set_footer_settings(ViewFooterSettings {
        lighthouse: false,
        ..classic()
    });
    assert!(sent(&setup.host, xs).and_then(|f| f.lighthouse).is_none());
    assert_eq!(setup.host.footer_height(xs), FOOTER_HEIGHT);
    assert_eq!(setup.host.footer_lighthouse_target(xs), None);
    // The breakpoint bar and the vitals bar off: the Lighthouse bar alone.
    setup.host.set_footer_settings(ViewFooterSettings {
        visible: false,
        vitals: false,
        ..classic()
    });
    let footer = sent(&setup.host, xs).expect("the Lighthouse bar alone");
    assert!(footer.bar_lines().is_empty());
    assert!(footer.lighthouse_shown);
    assert_eq!(setup.host.footer_height(xs), FOOTER_HEIGHT);
    let others: Vec<ViewId> = setup
        .host
        .views()
        .iter()
        .map(|view| view.id)
        .filter(|view| *view != xs)
        .collect();
    for view in others {
        assert_eq!(sent(&setup.host, view), None, "no bar at all: no footer");
        assert_eq!(setup.host.footer_height(view), 0);
    }
}

#[tokio::test]
async fn the_same_runs_again_write_nothing() {
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let xs = view_of(&setup.host, "xs");
    setup.host.footer_navigated(xs, PAGE);
    let runs = vec![run("20261117-100000", "xs", PAGE, 72)];
    setup.host.set_footer_lighthouse(runs.clone());
    let calls = setup.ui.calls().len();
    setup.host.set_footer_lighthouse(runs);
    assert_eq!(setup.ui.calls().len(), calls);
}

#[tokio::test]
async fn a_narrow_view_names_the_categories_short_or_not_at_all() {
    use crate::view_footer::lighthouse::Form;
    let setup = setup(Some(classic()));
    setup.host.on_grid_area(area());
    let xs = view_of(&setup.host, "xs");
    setup.host.footer_navigated(xs, PAGE);
    setup
        .host
        .set_footer_lighthouse(vec![run("20261117-100000", "xs", PAGE, 72)]);
    let form = sent(&setup.host, xs).map(|footer| footer.lighthouse_form);
    // Three views side by side in 1200 DIP: about 400 DIP each, too narrow for every name.
    assert!(
        matches!(form, Some(Form::Short | Form::Bare)),
        "the form fits the cell: {form:?}"
    );
}
