use super::locator;
use crate::model::{Locator, LocatorPreference, Selector};

fn testid(testid: &str, nth: u32) -> Selector {
    Selector::Testid {
        testid: testid.to_owned(),
        nth,
    }
}

fn role(role: &str, name: &str, nth: u32) -> Selector {
    seen_role(role, name, nth, None)
}

/// A role match as the agent records it since RB-090-81: with its index among the visible ones.
fn seen_role(role: &str, name: &str, nth: u32, visible_nth: Option<u32>) -> Selector {
    Selector::Role {
        role: role.to_owned(),
        name: name.to_owned(),
        nth,
        visible_nth,
        within: None,
    }
}

fn text(text: &str, nth: u32) -> Selector {
    seen_text(text, nth, None)
}

fn seen_text(text: &str, nth: u32, visible_nth: Option<u32>) -> Selector {
    Selector::Text {
        tag: "p".to_owned(),
        text: text.to_owned(),
        nth,
        visible_nth,
        within: None,
    }
}

fn css(path: &str) -> Selector {
    Selector::Css {
        path: path.to_owned(),
    }
}

fn of(selectors: Vec<Selector>) -> Locator {
    Locator {
        selectors,
        ..Locator::default()
    }
}

fn auto(target: &Locator) -> String {
    locator(target, LocatorPreference::Auto).expect("a locator")
}

#[test]
fn a_test_id_becomes_get_by_test_id() {
    assert_eq!(
        auto(&of(vec![testid("login", 0), css("#login")])),
        "page.getByTestId('login')"
    );
}

#[test]
fn a_role_becomes_get_by_role_with_its_exact_name() {
    assert_eq!(
        auto(&of(vec![
            role("button", "Sign in", 0),
            css("form > button")
        ])),
        "page.getByRole('button', { name: 'Sign in', exact: true }).filter({ visible: true }).first()"
    );
}

#[test]
fn a_role_without_a_name_has_no_options() {
    assert_eq!(
        auto(&of(vec![role("navigation", "", 0), css("nav")])),
        "page.getByRole('navigation').filter({ visible: true }).first()"
    );
}

#[test]
fn a_text_becomes_exact_get_by_text() {
    assert_eq!(
        auto(&of(vec![text("Welcome back", 0), css("main > p")])),
        "page.getByText('Welcome back', { exact: true }).filter({ visible: true }).first()"
    );
}

#[test]
fn a_css_path_is_the_fallback() {
    assert_eq!(
        auto(&of(vec![css(
            "#plain > fieldset > label:nth-child(2) > input"
        )])),
        "page.locator('#plain > fieldset > label:nth-child(2) > input')"
    );
}

#[test]
fn a_later_test_id_match_is_taken_by_nth() {
    assert_eq!(
        auto(&of(vec![testid("row", 2), css("li:nth-child(3)")])),
        "page.getByTestId('row').nth(2)"
    );
}

/// RB-090-81: a role or text locator counts the matches a view shows. The fourth of several
/// visible cards' "Read more" links stays the fourth; a desktop link behind a hidden mobile
/// menu is the first visible one, which is the mobile copy where the mobile menu shows.
#[test]
fn the_cards_case_takes_the_visible_match_the_person_clicked() {
    assert_eq!(
        auto(&of(vec![
            seen_role("link", "Read more", 3, Some(3)),
            css("a")
        ])),
        "page.getByRole('link', { name: 'Read more', exact: true }).filter({ visible: true }).nth(3)"
    );
    assert_eq!(
        auto(&of(vec![seen_text("Details", 2, Some(1)), css("p")])),
        "page.getByText('Details', { exact: true }).filter({ visible: true }).nth(1)"
    );
}

#[test]
fn the_hidden_nav_case_takes_the_first_visible_match() {
    assert_eq!(
        auto(&of(vec![
            seen_role("link", "Experienced", 1, Some(0)),
            css("a")
        ])),
        "page.getByRole('link', { name: 'Experienced', exact: true }).filter({ visible: true }).first()"
    );
    // The first match, unique or not, is the first visible one.
    assert_eq!(
        auto(&of(vec![role("link", "Sustainability", 0), css("a")])),
        "page.getByRole('link', { name: 'Sustainability', exact: true }).filter({ visible: true }).first()"
    );
    assert_eq!(
        auto(&of(vec![text("Read more", 0), css("p")])),
        "page.getByText('Read more', { exact: true }).filter({ visible: true }).first()"
    );
}

/// A later match without the visible index - a recording made before, or a hidden element -
/// keeps its document position.
#[test]
fn a_later_match_without_its_visible_index_keeps_nth() {
    assert_eq!(
        auto(&of(vec![role("link", "More", 1), css("a")])),
        "page.getByRole('link', { name: 'More', exact: true }).nth(1)"
    );
    assert_eq!(
        auto(&of(vec![text("Read more", 3), css("p")])),
        "page.getByText('Read more', { exact: true }).nth(3)"
    );
}

#[test]
fn a_role_playwright_does_not_know_falls_back_to_the_next_candidate() {
    assert_eq!(
        auto(&of(vec![role("fancy-widget", "Go", 0), css("#go")])),
        "page.locator('#go')"
    );
}

#[test]
fn the_preference_picks_its_kind_when_the_locator_has_it() {
    let target = of(vec![
        testid("login", 0),
        role("button", "Log in", 0),
        css("#login"),
    ]);
    assert_eq!(
        locator(&target, LocatorPreference::Css).expect("css"),
        "page.locator('#login')"
    );
    assert_eq!(
        locator(&target, LocatorPreference::Role).expect("role"),
        "page.getByRole('button', { name: 'Log in', exact: true }).filter({ visible: true }).first()"
    );
    assert_eq!(
        locator(&target, LocatorPreference::Text).expect("the first instead"),
        "page.getByTestId('login')"
    );
}

#[test]
fn frames_and_shadow_hosts_come_first() {
    let target = Locator {
        selectors: vec![testid("pay", 0), css("#pay")],
        hosts: vec![css("checkout-form")],
        frame: Some(vec!["iframe#shop".to_owned(), "iframe.inner".to_owned()]),
    };
    assert_eq!(
        auto(&target),
        "page.frameLocator('iframe#shop').frameLocator('iframe.inner')\
         .locator('checkout-form').getByTestId('pay')"
    );
}

#[test]
fn a_shadow_host_is_located_as_recorded_the_element_in_it_by_visibility() {
    let target = Locator {
        selectors: vec![role("button", "Pay", 0), css("#pay")],
        hosts: vec![role("region", "Checkout", 1)],
        frame: None,
    };
    assert_eq!(
        auto(&target),
        "page.getByRole('region', { name: 'Checkout', exact: true }).nth(1)\
         .getByRole('button', { name: 'Pay', exact: true }).filter({ visible: true }).first()"
    );
}

#[test]
fn a_locator_without_a_selector_cannot_be_exported() {
    assert!(locator(&of(Vec::new()), LocatorPreference::Auto).is_err());
}

#[test]
fn a_name_shown_twice_is_looked_for_inside_its_ancestor() {
    // RB-090-84: a mega menu's link and a content link of one name; the menu's comes with
    // `within`, so a view whose menu is not open yet waits for it instead of clicking the other.
    let mut menu = role("link", "Brands & Produkte", 0);
    if let Selector::Role { within, .. } = &mut menu {
        *within = Some("#main-nav".to_owned());
    }
    assert_eq!(
        auto(&of(vec![menu, css("#main-nav > ul > li > a")])),
        "page.locator('#main-nav').getByRole('link', { name: 'Brands & Produkte', exact: true })\
         .filter({ visible: true }).first()"
    );
    let mut offer = text("Offers", 0);
    if let Selector::Text { within, .. } = &mut offer {
        *within = Some("main".to_owned());
    }
    assert_eq!(
        auto(&of(vec![offer])),
        "page.locator('main').getByText('Offers', { exact: true }).filter({ visible: true }).first()"
    );
}

#[test]
fn recorded_text_cannot_break_out_of_the_literal() {
    assert_eq!(
        auto(&of(vec![css("a[title='x'); alert(1); ('")])),
        r"page.locator('a[title=\'x\'); alert(1); (\'')"
    );
}
