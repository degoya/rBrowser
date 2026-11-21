//! RB-090-76: the steps of a recording made on a navigation with a hover-only dropdown and a
//! checkbox that opens the mobile navigation - neutral names, the step order of the finding.
//! The click into the dropdown ran only in the wide breakpoints, the navigation it caused in
//! every view; a recording made before the fix filled the checkbox.

use super::{Context, Guard, render};
use crate::model::{Action, Locator, LocatorPreference, Selector, Step};

const START: &str = "http://127.0.0.1:8000/en/start";
const JOBS: &str = "http://127.0.0.1:8000/en/careers/jobs/experienced";
const WIDE: [&str; 3] = ["desktop", "w1920", "xl"];

fn by_role(role: &str, name: &str) -> Locator {
    Locator {
        selectors: vec![
            Selector::Role {
                role: role.to_owned(),
                name: name.to_owned(),
                nth: 0,
                visible_nth: None,
                within: None,
            },
            Selector::Css {
                path: format!("#{}", name.to_lowercase().replace(' ', "-")),
            },
        ],
        ..Locator::default()
    }
}

fn step(id: u32, action: Action, only_in: &[&str]) -> Step {
    Step {
        id,
        name: None,
        action,
        view: "desktop".to_owned(),
        only_in: only_in.iter().map(|id| (*id).to_owned()).collect(),
        at_ms: 0,
        path: Vec::new(),
        disabled: false,
        pause_ms: None,
    }
}

fn recording() -> Vec<Step> {
    let toggle = || by_role("checkbox", "Open the navigation");
    vec![
        step(
            1,
            Action::Navigate {
                url: START.to_owned(),
            },
            &[],
        ),
        step(
            2,
            Action::Hover {
                target: by_role("link", "Careers"),
            },
            &WIDE,
        ),
        step(
            3,
            Action::Click {
                target: by_role("link", "Experienced"),
                button: None,
                modifiers: 0,
            },
            &WIDE,
        ),
        step(
            4,
            Action::WaitForUrl {
                url: JOBS.to_owned(),
            },
            &[],
        ),
        step(
            5,
            Action::Fill {
                target: toggle(),
                value: "on".to_owned(),
                secret: false,
            },
            &["mobile"],
        ),
        step(6, Action::Check { target: toggle() }, &["mobile"]),
    ]
}

fn flow() -> String {
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    render(&recording(), &mut context, 0).expect("rendered")
}

fn spec() -> String {
    let mut context = Context::new(
        Guard::Project,
        LocatorPreference::Auto,
        Some("http://127.0.0.1:8000".to_owned()),
    );
    render(&recording(), &mut context, 1).expect("rendered")
}

#[test]
fn the_breakpoints_without_the_click_go_to_its_page_before_the_url_is_expected() {
    assert!(
        flow().contains(
            "// Step 4: waitForUrl\n\
             await step('Step 4: waitForUrl', async () => {\n  \
             if (!['desktop', 'w1920', 'xl'].includes(view.breakpoint)) {\n    \
             await page.goto('http://127.0.0.1:8000/en/careers/jobs/experienced');\n  }\n  \
             await expect(page).toHaveURL('http://127.0.0.1:8000/en/careers/jobs/experienced');\n\
             });\n"
        ),
        "{}",
        flow()
    );
    assert!(
        spec().contains(
            "  // Step 4: waitForUrl\n  \
             if (!['desktop', 'w1920', 'xl'].includes(test.info().project.name)) {\n    \
             await page.goto('/en/careers/jobs/experienced');\n  }\n  \
             await expect(page).toHaveURL('/en/careers/jobs/experienced');\n"
        ),
        "{}",
        spec()
    );
}

#[test]
fn a_navigation_after_a_click_in_every_breakpoint_needs_no_goto() {
    let mut steps = recording();
    steps[2].only_in.clear();
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    let text = render(&steps, &mut context, 0).expect("rendered");
    assert_eq!(text.matches("page.goto(").count(), 1, "{text}");
}

#[test]
fn the_hover_on_the_menu_entry_comes_before_the_click_into_its_dropdown() {
    let text = flow();
    let hover = text
        .find("getByRole('link', { name: 'Careers', exact: true }).filter({ visible: true }).first().hover()")
        .expect("the hover");
    let click = text
        .find("getByRole('link', { name: 'Experienced', exact: true }).filter({ visible: true }).first().click()")
        .expect("the click");
    assert!(hover < click, "{text}");
}

const TOGGLE: &str = "getByRole('checkbox', { name: 'Open the navigation', exact: true }).filter({ visible: true }).first()";

#[test]
fn a_legacy_checkbox_fill_before_its_check_is_left_out() {
    // The check after it carries the state.
    for text in [flow(), spec()] {
        assert!(!text.contains(".fill("), "{text}");
        assert_eq!(
            text.matches(&format!("{TOGGLE}.check();")).count(),
            1,
            "{text}"
        );
        assert!(
            text.contains("// Step 5: fill (only in mobile) - left out: step 6 sets the control\n"),
            "{text}"
        );
    }
}

#[test]
fn a_legacy_checkbox_fill_before_an_uncheck_does_not_check() {
    let mut steps = recording();
    steps[5].action = Action::Uncheck {
        target: by_role("checkbox", "Open the navigation"),
    };
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    let text = render(&steps, &mut context, 0).expect("rendered");
    assert!(!text.contains(".check()"), "{text}");
    assert_eq!(text.matches(&format!("{TOGGLE}.uncheck();")).count(), 1);
}

#[test]
fn a_legacy_checkbox_fill_without_a_check_after_it_checks() {
    let mut steps = recording();
    steps.truncate(5);
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    let text = render(&steps, &mut context, 0).expect("rendered");
    assert!(!text.contains(".fill("), "{text}");
    assert!(text.contains("// Step 5: check (only in mobile)"), "{text}");
    assert_eq!(text.matches(&format!("{TOGGLE}.check();")).count(), 1);
}

#[test]
fn breakpoints_that_clicked_their_own_way_need_no_goto() {
    // Desktop clicks the link of its menu, mobile opens its hamburger menu (and so on): both
    // navigated themselves; only the rest is taken there.
    let mut steps = recording();
    steps.insert(
        3,
        step(
            7,
            Action::Click {
                target: by_role("button", "Menu"),
                button: None,
                modifiers: 0,
            },
            &["mobile"],
        ),
    );
    steps[2].only_in = vec!["desktop".to_owned()];
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    let text = render(&steps, &mut context, 0).expect("rendered");
    assert!(
        text.contains("if (!['desktop', 'mobile'].includes(view.breakpoint)) {"),
        "{text}"
    );
    assert_eq!(text.matches("page.goto(").count(), 2, "{text}");
}

#[test]
fn every_flow_step_names_itself_for_the_result() {
    let text = flow();
    for id in [1, 2, 3, 4, 6] {
        assert!(
            text.contains(&format!("await step('Step {id}: ")),
            "step {id}: {text}"
        );
    }
    // Step 5, the legacy fill, is left out.
    assert_eq!(text.matches("\n});\n").count(), 5, "{text}");
    assert!(!spec().contains("await step("));
}

#[test]
fn the_recorded_pace_waits_before_each_step_and_after_a_menu_hover() {
    // RB-090-84: the menu's hover carries the time of the click it was reported with.
    let mut steps = recording();
    for (step, at_ms) in steps.iter_mut().zip([0, 3_000, 3_004, 3_900, 5_000, 5_000]) {
        step.at_ms = at_ms;
    }
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None)
        .paced(crate::pace::Pace::Recorded);
    let text = render(&steps, &mut context, 0).expect("rendered");
    for (id, wait) in [(2, 2_504), (3, 500), (4, 896)] {
        assert!(
            text.contains(&format!(
                "await step('Step {id}: {}', async () => {{\n  await page.waitForTimeout({wait});\n",
                ["hover", "click", "waitForUrl"][id - 2]
            )),
            "step {id}: {text}"
        );
    }
    // Step 5 is left out, step 6 waits the time since step 4.
    assert!(
        text.contains(
            "await step('Step 6: check', async () => {\n  await page.waitForTimeout(1100);\n"
        ),
        "{text}"
    );
    assert_eq!(text.matches("waitForTimeout").count(), 4, "{text}");
}

#[test]
fn the_least_pause_holds_before_every_step_but_the_first() {
    let mut steps = recording();
    for (step, at_ms) in steps.iter_mut().zip([0, 3_000, 3_004, 3_900, 5_000, 5_000]) {
        step.at_ms = at_ms;
    }
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None)
        .paced(crate::pace::Pace::Recorded)
        .least_pause(1_000);
    let text = render(&steps, &mut context, 0).expect("rendered");
    // Hover 3 s; click and waitForUrl 1 s each (the click 4 ms after the hover); check 1.1 s.
    for wait in [3_000, 1_000, 1_100] {
        assert!(
            text.contains(&format!("await page.waitForTimeout({wait});")),
            "{wait}: {text}"
        );
    }
    assert_eq!(text.matches("waitForTimeout").count(), 4, "{text}");
}
