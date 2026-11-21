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
    tagged_text("p", text, nth, visible_nth)
}

fn tagged_text(tag: &str, text: &str, nth: u32, visible_nth: Option<u32>) -> Selector {
    Selector::Text {
        tag: tag.to_owned(),
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
fn a_text_becomes_its_tag_with_exactly_that_whole_text() {
    assert_eq!(
        auto(&of(vec![text("Welcome back", 0), css("main > p")])),
        r"page.locator('p').filter({ hasText: /^\s*Welcome\s+back\s*$/ }).filter({ visible: true }).first()"
    );
}

/// RB-090-93 review: the matches are the agent's own (`matchAll`): the elements of the tag
/// whose whole text is the text. Climbing from `getByText` to the tag took a `p` whose inner
/// `span` alone had the text, and an inner `p` of two nested ones, so `nth` counted others.
#[test]
fn a_text_does_not_match_an_element_that_only_contains_it() {
    let target = auto(&of(vec![tagged_text("label", "Menu", 1, None)]));
    assert_eq!(
        target,
        r"page.locator('label').filter({ hasText: /^\s*Menu\s*$/ }).nth(1)"
    );
    assert!(!target.contains("getByText") && !target.contains("ancestor"));
}

/// Page text reaches the regular expression escaped: nothing in it ends the literal or reads
/// as a pattern.
#[test]
fn a_text_with_pattern_characters_is_escaped_in_its_expression() {
    assert_eq!(
        auto(&of(vec![tagged_text("a", "1/2 price (today) *", 0, None)])),
        r"page.locator('a').filter({ hasText: /^\s*1\/2\s+price\s+\(today\)\s+\*\s*$/ }).filter({ visible: true }).first()"
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
        r"page.locator('p').filter({ hasText: /^\s*Details\s*$/ }).filter({ visible: true }).nth(1)"
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
        r"page.locator('p').filter({ hasText: /^\s*Read\s+more\s*$/ }).filter({ visible: true }).first()"
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
        r"page.locator('p').filter({ hasText: /^\s*Read\s+more\s*$/ }).nth(3)"
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
        r"page.locator('main').locator('p').filter({ hasText: /^\s*Offers\s*$/ }).filter({ visible: true }).first()"
    );
}

#[test]
fn recorded_text_cannot_break_out_of_the_literal() {
    assert_eq!(
        auto(&of(vec![css("a[title='x'); alert(1); ('")])),
        r"page.locator('a[title=\'x\'); alert(1); (\'')"
    );
}

/// RB-090-93: a burger label whose only text is a visually hidden span. `getByText` alone
/// finds the 1x1 span, which the label covers, and the click waits for good; the recorded
/// tag takes the label whose text it is.
#[test]
fn a_text_with_its_tag_clicks_the_element_of_that_tag_not_the_hidden_span_in_it() {
    assert_eq!(
        auto(&of(vec![
            tagged_text("label", "Menu", 0, None),
            css("header > label")
        ])),
        r"page.locator('label').filter({ hasText: /^\s*Menu\s*$/ }).filter({ visible: true }).first()"
    );
    assert_eq!(
        auto(&of(vec![tagged_text("label", "Menu", 2, Some(1))])),
        r"page.locator('label').filter({ hasText: /^\s*Menu\s*$/ }).filter({ visible: true }).nth(1)"
    );
    assert_eq!(
        auto(&of(vec![tagged_text("my-toggle", "Menu", 1, None)])),
        r"page.locator('my-toggle').filter({ hasText: /^\s*Menu\s*$/ }).nth(1)"
    );
}

/// The text directly in the element: the same locator.
#[test]
fn a_text_directly_in_its_element_is_that_element() {
    assert_eq!(
        auto(&of(vec![tagged_text("button", "Save", 0, None)])),
        r"page.locator('button').filter({ hasText: /^\s*Save\s*$/ }).filter({ visible: true }).first()"
    );
}

/// A tag that is no plain lower-case name (an SVG `foreignObject`, a forged recording) stays
/// out of the code; the locator is the text alone, as before.
#[test]
fn a_tag_that_is_no_plain_name_is_left_out() {
    for tag in ["", "foreignObject", "1p", "p]|//a", "p[1]", "-x", "label "] {
        assert_eq!(
            auto(&of(vec![tagged_text(tag, "Menu", 0, None)])),
            "page.getByText('Menu', { exact: true }).filter({ visible: true }).first()",
            "tag {tag:?}"
        );
    }
}

/// The tag composes with the ancestor that makes the text unique, frames and shadow hosts.
#[test]
fn a_tagged_text_inside_its_ancestor_frame_and_host() {
    let mut menu = tagged_text("label", "Menu", 0, None);
    if let Selector::Text { within, .. } = &mut menu {
        *within = Some("header".to_owned());
    }
    let target = Locator {
        selectors: vec![menu, css("header > label")],
        hosts: vec![css("site-header")],
        frame: Some(vec!["iframe#app".to_owned()]),
    };
    assert_eq!(
        auto(&target),
        r"page.frameLocator('iframe#app').locator('site-header').locator('header').locator('label').filter({ hasText: /^\s*Menu\s*$/ }).filter({ visible: true }).first()"
    );
}
