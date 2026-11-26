use super::*;
use crate::settings_window::guard::query_allowed;

/// A distinct id per page: its place in [`ShellPage::ALL`] plus one (0 is no browser).
fn id_by_place(page: ShellPage) -> i32 {
    let place = ShellPage::ALL
        .iter()
        .position(|candidate| *candidate == page)
        .expect("every page is listed");
    i32::try_from(place).expect("a few pages") + 1
}

#[test]
fn every_page_s_browser_lands_in_its_own_field() {
    let known = known_browsers(id_by_place);
    assert_eq!(
        [
            known.shell,
            known.pages,
            known.palette,
            known.panels,
            known.panels_dock,
            known.history,
            known.status_bar,
            known.board,
            known.gallery,
        ],
        [1, 2, 3, 4, 5, 6, 7, 8, 9]
    );
}

#[test]
fn each_page_s_browser_may_query_and_no_other() {
    let known = known_browsers(id_by_place);
    let from = |browser: i32| {
        query_allowed(
            QueryOrigin {
                browser,
                main_frame: true,
                url: "rbrowser://ui/index.html",
            },
            known,
        )
    };
    for page in ShellPage::ALL {
        assert!(from(id_by_place(page)), "{page:?}");
    }
    let unknown = i32::try_from(ShellPage::ALL.len()).expect("a few pages") + 1;
    assert!(!from(unknown));
    assert!(!from(0));
}

#[test]
fn a_page_without_a_browser_lets_nobody_in() {
    // No window open but the shell: the other fields stay 0, and 0 is never a browser.
    let known = known_browsers(|page| i32::from(page == ShellPage::Shell) * 7);
    assert_eq!(known.shell, 7);
    assert_eq!(known.gallery, 0);
    let origin = QueryOrigin {
        browser: 0,
        main_frame: true,
        url: "rbrowser://ui/index.html",
    };
    assert!(!query_allowed(origin, known));
}
