//! One snapshot per step kind, for the spec and for the flow.

use super::{Context, Guard, render};
use crate::model::{Action, Assertion, Locator, LocatorPreference, Selector, Step};

fn button() -> Locator {
    Locator {
        selectors: vec![
            Selector::Testid {
                testid: "login".to_owned(),
                nth: 0,
            },
            Selector::Css {
                path: "#login".to_owned(),
            },
        ],
        ..Locator::default()
    }
}

fn step(id: u32, action: Action) -> Step {
    Step {
        id,
        name: None,
        action,
        view: "desktop".to_owned(),
        only_in: Vec::new(),
        at_ms: 0,
    }
}

fn spec_context() -> Context {
    Context::new(
        Guard::Project,
        LocatorPreference::Auto,
        Some("http://127.0.0.1:8000".to_owned()),
    )
}

fn spec(action: Action) -> String {
    render(&[step(7, action)], &mut spec_context(), 1).expect("rendered")
}

fn flow(action: Action) -> String {
    let mut context = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    render(&[step(7, action)], &mut context, 0).expect("rendered")
}

#[test]
fn navigate() {
    let action = || Action::Navigate {
        url: "http://127.0.0.1:8000/recorder.html".to_owned(),
    };
    assert_eq!(
        spec(action()),
        "  // Step 7: navigate\n  await page.goto('/recorder.html');\n"
    );
    assert_eq!(
        flow(action()),
        "// Step 7: navigate\nawait step('Step 7: navigate', async () => {\n  \
         await page.goto('http://127.0.0.1:8000/recorder.html');\n});\n"
    );
}

#[test]
fn click() {
    assert_eq!(
        spec(Action::Click {
            target: button(),
            button: None,
            modifiers: 0
        }),
        "  // Step 7: click\n  await page.getByTestId('login').click();\n"
    );
    assert_eq!(
        spec(Action::Click {
            target: button(),
            button: Some("right".to_owned()),
            modifiers: 2 | 8
        }),
        "  // Step 7: click\n  await page.getByTestId('login').click({ button: 'right', \
         modifiers: ['Control', 'Shift'] });\n"
    );
}

#[test]
fn dblclick() {
    assert_eq!(
        spec(Action::Dblclick { target: button() }),
        "  // Step 7: dblclick\n  await page.getByTestId('login').dblclick();\n"
    );
}

#[test]
fn fill() {
    assert_eq!(
        spec(Action::Fill {
            target: button(),
            value: "alice".to_owned(),
            secret: false
        }),
        "  // Step 7: fill\n  await page.getByTestId('login').fill('alice');\n"
    );
}

#[test]
fn press() {
    assert_eq!(
        spec(Action::Press {
            target: Some(button()),
            key: "Enter".to_owned(),
            modifiers: 0
        }),
        "  // Step 7: press\n  await page.getByTestId('login').press('Enter');\n"
    );
    assert_eq!(
        spec(Action::Press {
            target: None,
            key: "k".to_owned(),
            modifiers: 1 | 4
        }),
        "  // Step 7: press\n  await page.keyboard.press('Alt+Meta+k');\n"
    );
}

#[test]
fn select() {
    let select = |value: Option<&str>, indexes: Vec<u32>| {
        spec(Action::Select {
            target: button(),
            value: value.map(str::to_owned),
            indexes,
        })
    };
    assert_eq!(
        select(Some("m"), vec![1]),
        "  // Step 7: select\n  await page.getByTestId('login').selectOption('m');\n"
    );
    assert_eq!(
        select(Some("cheese"), vec![0, 2]),
        "  // Step 7: select\n  await page.getByTestId('login').selectOption([{ index: 0 }, \
         { index: 2 }]);\n"
    );
    assert_eq!(
        select(None, vec![3]),
        "  // Step 7: select\n  await page.getByTestId('login').selectOption({ index: 3 });\n"
    );
    assert_eq!(
        select(None, Vec::new()),
        "  // Step 7: select\n  await page.getByTestId('login').selectOption([]);\n"
    );
}

#[test]
fn check_and_uncheck() {
    assert_eq!(
        spec(Action::Check { target: button() }),
        "  // Step 7: check\n  await page.getByTestId('login').check();\n"
    );
    assert_eq!(
        spec(Action::Uncheck { target: button() }),
        "  // Step 7: uncheck\n  await page.getByTestId('login').uncheck();\n"
    );
}

#[test]
fn hover() {
    assert_eq!(
        spec(Action::Hover { target: button() }),
        "  // Step 7: hover\n  await page.getByTestId('login').hover();\n"
    );
}

#[test]
fn scroll() {
    assert_eq!(
        spec(Action::Scroll {
            target: None,
            x: 0.0,
            y: 640.5
        }),
        "  // Step 7: scroll\n  await page.evaluate(() => window.scrollTo(0, 640.5));\n"
    );
    assert_eq!(
        spec(Action::Scroll {
            target: Some(button()),
            x: 10.0,
            y: 20.0
        }),
        "  // Step 7: scroll\n  await page.getByTestId('login').evaluate((element) => \
         element.scrollTo(10, 20));\n"
    );
}

#[test]
fn wait_for_url() {
    assert_eq!(
        spec(Action::WaitForUrl {
            url: "http://127.0.0.1:8000/welcome.html".to_owned()
        }),
        "  // Step 7: waitForUrl\n  await expect(page).toHaveURL('/welcome.html');\n"
    );
}

#[test]
fn wait_for_an_element_in_the_spec_and_the_flow() {
    // The page menu's "Wait for element" (RB-090-60).
    let action = || Action::WaitFor { target: button() };
    assert_eq!(
        spec(action()),
        "  // Step 7: waitFor\n  await page.getByTestId('login').waitFor();\n"
    );
    assert_eq!(
        flow(action()),
        "// Step 7: waitFor\nawait step('Step 7: waitFor', async () => {\n  \
         await page.getByTestId('login').waitFor();\n});\n"
    );
}

#[test]
fn the_page_menu_checks_land_in_the_flow_too() {
    // "Check: element visible" and "Check: text visible" (RB-090-60) in an rBrowser flow.
    assert_eq!(
        flow(Action::Assert {
            assertion: Assertion::Visible { target: button() }
        }),
        "// Step 7: assert visible\nawait step('Step 7: assert visible', async () => {\n  \
         await expect(page.getByTestId('login')).toBeVisible();\n});\n"
    );
    assert_eq!(
        flow(Action::Assert {
            assertion: Assertion::Text {
                target: button(),
                text: "Log in".to_owned()
            }
        }),
        "// Step 7: assert text\nawait step('Step 7: assert text', async () => {\n  \
         await expect(page.getByTestId('login')).toHaveText('Log in');\n});\n"
    );
}

#[test]
fn every_assertion() {
    let assert = |assertion| spec(Action::Assert { assertion });
    assert_eq!(
        assert(Assertion::Visible { target: button() }),
        "  // Step 7: assert visible\n  await expect(page.getByTestId('login')).toBeVisible();\n"
    );
    assert_eq!(
        assert(Assertion::Text {
            target: button(),
            text: "Log in".to_owned()
        }),
        "  // Step 7: assert text\n  await expect(page.getByTestId('login')).toHaveText('Log in');\n"
    );
    assert_eq!(
        assert(Assertion::Value {
            target: button(),
            value: "{{secret:password}}".to_owned()
        }),
        // An assertion never reads a secret: the placeholder is text.
        "  // Step 7: assert value\n  await expect(page.getByTestId('login'))\
         .toHaveValue('{{secret:password}}');\n"
    );
    assert_eq!(
        assert(Assertion::Url {
            url: "http://127.0.0.1:8000/".to_owned()
        }),
        "  // Step 7: assert url\n  await expect(page).toHaveURL('/');\n"
    );
}

#[test]
fn a_named_step_carries_its_name_in_the_comment() {
    let mut named = step(3, Action::Hover { target: button() });
    named.name = Some("Open the\nmenu".to_owned());
    let text = render(&[named], &mut spec_context(), 0).expect("rendered");
    assert!(
        text.starts_with("// Step 3: hover - Open the menu\n"),
        "{text}"
    );
}

#[test]
fn an_only_in_step_is_guarded_by_the_project_name_in_the_spec() {
    let mut only = step(4, Action::Check { target: button() });
    only.only_in = vec!["mobile".to_owned(), "tablet".to_owned()];
    assert_eq!(
        render(&[only.clone()], &mut spec_context(), 1).expect("rendered"),
        "  // Step 4: check (only in mobile, tablet)\n  \
         if (['mobile', 'tablet'].includes(test.info().project.name)) {\n    \
         await page.getByTestId('login').check();\n  }\n"
    );
    let mut flow = Context::new(Guard::Breakpoint, LocatorPreference::Auto, None);
    assert_eq!(
        render(&[only], &mut flow, 0).expect("rendered"),
        "// Step 4: check (only in mobile, tablet)\n\
         await step('Step 4: check', async () => {\n  \
         if (['mobile', 'tablet'].includes(view.breakpoint)) {\n    \
         await page.getByTestId('login').check();\n  }\n});\n"
    );
}

#[test]
fn a_secret_is_an_environment_variable_and_is_listed() {
    let mut context = spec_context();
    let text = render(
        &[step(
            5,
            Action::Fill {
                target: button(),
                value: "{{secret:password}}".to_owned(),
                secret: true,
            },
        )],
        &mut context,
        0,
    )
    .expect("rendered");
    assert!(
        text.contains(".fill(rbSecret('RB_SECRET_PASSWORD'));"),
        "{text}"
    );
    assert!(!text.contains("{{secret"), "{text}");
    assert_eq!(context.secrets.names(), ["RB_SECRET_PASSWORD"]);
}

/// RB-050-02 review: only a fill the recorder marked secret reads the environment; a
/// placeholder in any other text - typed, from the page, in a URL, an assertion or an option -
/// is a plain literal, so no page can make the code send a secret anywhere.
#[test]
fn a_placeholder_outside_a_secret_fill_stays_text() {
    let placeholder = "{{secret:admin}}";
    let at = |path: &str| format!("http://evil.test/{path}?k={placeholder}");
    let steps = [
        Action::Fill {
            target: button(),
            value: placeholder.to_owned(),
            secret: false,
        },
        Action::Navigate { url: at("go") },
        Action::WaitForUrl { url: at("wait") },
        Action::Select {
            target: button(),
            value: Some(placeholder.to_owned()),
            indexes: vec![1],
        },
        Action::Assert {
            assertion: Assertion::Text {
                target: button(),
                text: placeholder.to_owned(),
            },
        },
        Action::Assert {
            assertion: Assertion::Value {
                target: button(),
                value: placeholder.to_owned(),
            },
        },
        Action::Assert {
            assertion: Assertion::Url { url: at("url") },
        },
    ];
    let steps: Vec<Step> = steps
        .into_iter()
        .enumerate()
        .map(|(i, action)| step(i as u32 + 1, action))
        .collect();
    for mut context in [
        spec_context(),
        Context::new(Guard::Breakpoint, LocatorPreference::Auto, None),
    ] {
        let text = render(&steps, &mut context, 0).expect("rendered");
        assert!(!text.contains("process.env"), "{text}");
        assert!(!text.contains("rbSecret"), "{text}");
        assert!(context.secrets.names().is_empty(), "{text}");
        assert_eq!(text.matches(placeholder).count(), 7, "{text}");
    }
}

/// A fill marked secret that holds no placeholder is refused rather than written.
#[test]
fn a_secret_fill_without_a_placeholder_is_refused() {
    for value in [
        "hunter2",
        "{{secret:a b}}",
        "x{{secret:pw}}",
        "{{secret:pw}}{{secret:pw}}",
    ] {
        let error = render(
            &[step(
                3,
                Action::Fill {
                    target: button(),
                    value: value.to_owned(),
                    secret: true,
                },
            )],
            &mut spec_context(),
            0,
        )
        .expect_err("refused");
        assert_eq!(error.step, 3);
        assert!(!error.reason.contains("hunter2"), "{}", error.reason);
    }
}

#[test]
fn a_step_without_a_usable_selector_names_its_id() {
    let error = render(
        &[step(
            9,
            Action::Click {
                target: Locator::default(),
                button: None,
                modifiers: 0,
            },
        )],
        &mut spec_context(),
        0,
    )
    .expect_err("no selector");
    assert_eq!(error.step, 9);
}
