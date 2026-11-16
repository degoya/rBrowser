use super::locator;
use crate::model::{Locator, LocatorPreference, Selector};

fn testid(testid: &str, nth: u32) -> Selector {
    Selector::Testid {
        testid: testid.to_owned(),
        nth,
    }
}

fn role(role: &str, name: &str, nth: u32) -> Selector {
    Selector::Role {
        role: role.to_owned(),
        name: name.to_owned(),
        nth,
    }
}

fn text(text: &str, nth: u32) -> Selector {
    Selector::Text {
        tag: "p".to_owned(),
        text: text.to_owned(),
        nth,
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
        "page.getByRole('button', { name: 'Sign in', exact: true })"
    );
}

#[test]
fn a_role_without_a_name_has_no_options() {
    assert_eq!(
        auto(&of(vec![role("navigation", "", 0), css("nav")])),
        "page.getByRole('navigation')"
    );
}

#[test]
fn a_text_becomes_exact_get_by_text() {
    assert_eq!(
        auto(&of(vec![text("Welcome back", 0), css("main > p")])),
        "page.getByText('Welcome back', { exact: true })"
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
fn a_later_match_is_taken_by_nth() {
    assert_eq!(
        auto(&of(vec![testid("row", 2), css("li:nth-child(3)")])),
        "page.getByTestId('row').nth(2)"
    );
    assert_eq!(
        auto(&of(vec![role("link", "More", 1), css("a")])),
        "page.getByRole('link', { name: 'More', exact: true }).nth(1)"
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
        "page.getByRole('button', { name: 'Log in', exact: true })"
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
fn a_locator_without_a_selector_cannot_be_exported() {
    assert!(locator(&of(Vec::new()), LocatorPreference::Auto).is_err());
}

#[test]
fn recorded_text_cannot_break_out_of_the_literal() {
    assert_eq!(
        auto(&of(vec![css("a[title='x'); alert(1); ('")])),
        r"page.locator('a[title=\'x\'); alert(1); (\'')"
    );
}
