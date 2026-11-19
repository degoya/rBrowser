use super::*;
use crate::recorder::model::{Recording, Selector};
use crate::recorder::record::{Recorder, RecorderOptions};
use crate::recorder::testing::*;
use crate::recorder::to_json;

const THIRD: ViewId = ViewId(3);

fn field(element: crate::event::ElementSelector, kind: AutofillKind, value: &str) -> AutofillField {
    AutofillField {
        target: at(element),
        kind,
        value: Some(value.to_owned()),
        selected: Vec::new(),
        sensitive: false,
    }
}

fn email() -> AutofillField {
    field(
        role("textbox", "Email", "#email"),
        AutofillKind::Fill,
        "ada@example.com",
    )
}

fn terms() -> AutofillField {
    AutofillField {
        value: None,
        ..field(
            role("checkbox", "I agree", "#terms"),
            AutofillKind::Check,
            "",
        )
    }
}

fn country() -> AutofillField {
    AutofillField {
        selected: vec![2],
        ..field(
            role("combobox", "Country", "#country"),
            AutofillKind::Select,
            "de",
        )
    }
}

fn filled(view: ViewId, fields: Vec<AutofillField>) -> AutofillView {
    AutofillView { view, fields }
}

/// The steps as `<type> <css path> [<only in>]`.
fn kinds(recording: &Recording) -> Vec<String> {
    recording
        .steps
        .iter()
        .map(|step| {
            let json = serde_json::to_value(&step.action).expect("json");
            let path = match &step.action {
                Action::Fill { target, .. }
                | Action::Check { target }
                | Action::Uncheck { target }
                | Action::Select { target, .. }
                | Action::Click { target, .. } => target.css_path().to_owned(),
                _ => String::new(),
            };
            format!(
                "{} {path} {:?}",
                json["type"].as_str().unwrap_or_default(),
                step.only_in
            )
        })
        .collect()
}

#[test]
fn a_fill_while_recording_is_a_step_per_field_with_its_value_in_the_views_that_filled_it() {
    let mut recorder = started(RecorderOptions::default(), None);
    let fields = || vec![email(), terms(), country()];
    assert!(recorder.on_autofill(&filled(DESKTOP, fields()), START + 500));
    // The fill's post of the next view joins the steps: same targets, same values.
    assert!(recorder.on_autofill(&filled(MOBILE, fields()), START + 520));
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        kinds(&recording),
        ["fill #email []", "check #terms []", "select #country []"]
    );
    match &recording.steps[0].action {
        Action::Fill {
            target,
            value,
            secret,
        } => {
            assert_eq!(value, "ada@example.com");
            assert!(!secret);
            // The best selector first, as the agent's targets give it.
            assert_eq!(
                target.selectors[0],
                Selector::Role {
                    role: "textbox".into(),
                    name: "Email".into(),
                    nth: 0,
                    visible_nth: None,
                    within: None,
                }
            );
        }
        other => panic!("not a fill: {other:?}"),
    }
    assert_eq!(
        recording.steps[2].action,
        Action::Select {
            target: Locator::from_target(&at(role("combobox", "Country", "#country")), None),
            value: Some("de".into()),
            indexes: vec![2],
        }
    );
    assert_eq!(recording.steps[0].view, "desktop");
    assert_eq!(recording.steps[0].at_ms, 500);
}

#[test]
fn a_view_the_fill_left_out_or_set_otherwise_has_its_own_steps() {
    let mut recorder = Recorder::new(RecorderOptions::default());
    let mut views = views();
    views.push(crate::recorder::record::RecordingView {
        view: THIRD,
        breakpoint: crate::recorder::model::RecordedBreakpoint {
            id: "firefox".into(),
            width: 800,
            height: 600,
            device_scale_factor: 1.0,
        },
    });
    recorder
        .start(crate::recorder::record::StartInfo {
            name: "fill".into(),
            start_url: None,
            environment: None,
            app_version: "0.1.0".into(),
            views,
            leader: Some(DESKTOP),
            now_ms: START,
        })
        .expect("starts");
    // A German view gets a German phone number; the third view (Firefox) is not filled.
    let phone = |value: &str| {
        field(
            role("textbox", "Phone", "#phone"),
            AutofillKind::Fill,
            value,
        )
    };
    recorder.on_autofill(
        &filled(DESKTOP, vec![email(), phone("+1 202 555 0143")]),
        START + 10,
    );
    recorder.on_autofill(
        &filled(MOBILE, vec![email(), phone("+49 30 1234567")]),
        START + 20,
    );
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        kinds(&recording),
        [
            "fill #email [\"desktop\", \"mobile\"]",
            "fill #phone [\"desktop\"]",
            "fill #phone [\"mobile\"]"
        ]
    );
}

#[test]
fn nothing_is_recorded_without_a_running_recording_or_with_the_setting_off() {
    let mut idle = Recorder::new(RecorderOptions::default());
    assert!(!idle.on_autofill(&filled(DESKTOP, vec![email()]), START));
    assert!(idle.stop().is_err());

    let mut paused = started(RecorderOptions::default(), None);
    paused.pause().expect("pauses");
    assert!(!paused.on_autofill(&filled(DESKTOP, vec![email()]), START + 10));
    assert_eq!(paused.step_count(), 0);

    let off = RecorderOptions {
        record_autofill: false,
        ..RecorderOptions::default()
    };
    let mut recorder = started(off, None);
    assert!(!recorder.on_autofill(&filled(DESKTOP, vec![email()]), START + 10));
    assert!(recorder.stop().expect("stops").steps.is_empty());
}

#[test]
fn a_password_and_a_configured_secret_field_are_recorded_as_their_placeholders() {
    let options = RecorderOptions {
        secret_fields: vec!["#otp".into()],
        ..RecorderOptions::default()
    };
    let mut recorder = started(options, None);
    let password = AutofillField {
        value: None,
        sensitive: true,
        ..field(css("form > input#password"), AutofillKind::Fill, "")
    };
    // A secret field the page script reports with a value: the value never reaches a step.
    let otp = field(css("#otp"), AutofillKind::Fill, PASSWORD);
    recorder.on_autofill(&filled(DESKTOP, vec![password, otp]), START + 10);
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        recording.steps[0].action,
        Action::Fill {
            target: Locator::from_target(&at(css("form > input#password")), None),
            value: "{{secret:password}}".into(),
            secret: true,
        }
    );
    match &recording.steps[1].action {
        Action::Fill { value, secret, .. } => {
            assert_eq!(value, "{{secret:otp}}");
            assert!(secret);
        }
        other => panic!("not a fill: {other:?}"),
    }
    assert!(!to_json(&recording).expect("json").contains(PASSWORD));
}

#[test]
fn the_agents_record_of_the_autofills_click_on_a_checkbox_is_no_second_step() {
    // The autofill clicks a checkbox; the browser reports that click's `change` as trusted, so the
    // agent records it in the active view - before the autofill's post or after it.
    let mut recorder = started(RecorderOptions::default(), None);
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        300,
        checked(role("checkbox", "I agree", "#terms"), true),
    );
    recorder.on_autofill(&filled(DESKTOP, vec![email(), terms()]), START + 400);
    recorder.on_autofill(&filled(MOBILE, vec![email(), terms()]), START + 410);
    let news = || AutofillField {
        value: None,
        ..field(
            role("checkbox", "Newsletter", "#news"),
            AutofillKind::Check,
            "",
        )
    };
    recorder.on_autofill(&filled(MOBILE, vec![news()]), START + 2_000);
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        2_100,
        checked(role("checkbox", "Newsletter", "#news"), true),
    );
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        kinds(&recording),
        [
            "fill #email []",
            "check #terms []",
            "check #news [\"mobile\"]"
        ]
    );
}

#[test]
fn a_persons_own_check_before_the_fill_stays() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        100,
        checked(role("checkbox", "I agree", "#terms"), true),
    );
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        200,
        click(role("button", "Next", "#next")),
    );
    recorder.on_autofill(&filled(DESKTOP, vec![terms()]), START + 400);
    // Later than the echo's window: a person's click again.
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        9_000,
        checked(role("checkbox", "I agree", "#terms"), false),
    );
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        9_100,
        checked(role("checkbox", "I agree", "#terms"), true),
    );
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        kinds(&recording),
        [
            "check #terms []",
            "click #next []",
            "check #terms [\"desktop\"]",
            "uncheck #terms []",
            "check #terms []"
        ]
    );
}

#[test]
fn a_post_from_the_page_script_reads_as_the_agents_target_shape() {
    let fields: Vec<AutofillField> = serde_json::from_value(serde_json::json!([
        { "target": { "element": { "kind": "testid", "testid": "email", "path": "#email" },
                      "offset": { "x": 0.5, "y": 0.5 } },
          "kind": "fill", "value": "ada@example.com" },
        { "target": { "element": { "kind": "css", "path": "#pw" }, "offset": { "x": 0.5, "y": 0.5 } },
          "kind": "fill", "sensitive": true },
        { "target": { "element": { "kind": "role", "role": "combobox", "name": "Country", "path": "#c" },
                      "offset": { "x": 0.5, "y": 0.5 } },
          "kind": "select", "value": "de", "selected": [2] },
        { "target": { "element": { "kind": "role", "role": "radio", "name": "Card", "path": "#card" },
                      "offset": { "x": 0.5, "y": 0.5 } },
          "kind": "uncheck" }
    ]))
    .expect("reads");
    assert_eq!(fields[0].kind, AutofillKind::Fill);
    assert!(fields[1].sensitive && fields[1].value.is_none());
    assert_eq!(fields[2].selected, [2]);
    assert_eq!(fields[3].kind, AutofillKind::Uncheck);
    assert!(
        serde_json::from_value::<AutofillField>(serde_json::json!({
            "target": { "element": { "kind": "css", "path": "#x" }, "offset": { "x": 0, "y": 0 } },
            "kind": "click"
        }))
        .is_err(),
        "only the four kinds"
    );
}
