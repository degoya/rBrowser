use super::*;

fn step(view: u32, name: &str, state: &str) -> FlowStep {
    FlowStep {
        run_id: "run-1".to_owned(),
        flow: "login".to_owned(),
        view,
        name: name.to_owned(),
        step_id: None,
        state: state.to_owned(),
        message: None,
        number: None,
        total: None,
    }
}

#[test]
fn the_steps_of_a_flow_are_counted_from_its_text() {
    let source = "// @views mobile\n\
        // step('commented out', () => {})\n\
        await step('log in', async () => {\n\
          await step ('fill', async () => {});\n\
        });\n\
        await test.step('not ours', async () => {});\n\
        const stepper = 1; mystep(2);\n\
        await step(\"Step 3: click\", async () => {}); /* step(x) */\n";
    assert_eq!(total_steps(source), Some(3));
    assert_eq!(
        total_steps("await page.goto('/');"),
        None,
        "no step, no total"
    );
}

/// RB-090-68: each view counts the steps it started; a passed or failed step keeps the number of
/// the last one started, and the total of the flow's text rides along.
#[test]
fn each_view_counts_its_steps() {
    let mut steps = RunSteps::new("run-1");
    steps.set_total(Some(3));
    let mut first = step(1, "log in", "running");
    steps.on_step(&mut first);
    assert_eq!((first.number, first.total), (Some(1), Some(3)));
    let mut other = step(2, "log in", "running");
    steps.on_step(&mut other);
    assert_eq!(other.number, Some(1), "views count on their own");
    let mut inner = step(1, "fill", "running");
    steps.on_step(&mut inner);
    let mut done = step(1, "fill", "passed");
    steps.on_step(&mut done);
    assert_eq!((done.number, done.total), (None, Some(3)));
    assert_eq!(
        steps.at(1),
        Some(FlowStepAt {
            number: 2,
            name: "fill".to_owned()
        })
    );
    assert_eq!(steps.at(3), None);
}

/// The tracker answers only for its run: a new run starts from nothing.
#[test]
fn a_new_run_starts_from_nothing() {
    begin("run-a");
    set_total("run-a", Some(2));
    let mut first = step(1, "a", "running");
    note("run-a", &mut first);
    assert_eq!(first.number, Some(1));
    let (total, views) = current("run-a").expect("run-a is tracked");
    assert_eq!(total, Some(2));
    assert_eq!(views.get(&1).map(|at| at.number), Some(1));
    assert!(current("run-b").is_none());
    begin("run-b");
    assert!(current("run-a").is_none());
    let (total, views) = current("run-b").expect("run-b is tracked");
    assert_eq!((total, views.len()), (None, 0));
    let mut stray = step(1, "a", "running");
    note("run-a", &mut stray);
    assert_eq!(stray.number, None, "a step of another run is not counted");
}
