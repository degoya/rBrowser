use super::*;
use crate::event::MOD_SHIFT;
use crate::recorder::model::{Action, Locator, Selector};
use crate::recorder::testing::*;
use crate::recorder::to_json;

fn actions(recording: &Recording) -> Vec<&Action> {
    recording.steps.iter().map(|step| &step.action).collect()
}

#[test]
fn ten_keystrokes_into_a_field_are_one_fill() {
    let mut recorder = started(RecorderOptions::default(), None);
    let name = css("#name");
    let typed = "Ada Lovela";
    for (i, c) in typed.char_indices() {
        let ms = 100 + i as u64 * 50;
        let modifiers = if c.is_uppercase() { MOD_SHIFT } else { 0 };
        let text = c.to_string();
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms,
            key(&text, modifiers, Some(name.clone())),
        );
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms + 1,
            input(name.clone(), &typed[..i + c.len_utf8()], false),
        );
    }
    let recording = recorder.stop().expect("stops");
    assert_eq!(recording.steps.len(), 1, "{:?}", recording.steps);
    match &recording.steps[0].action {
        Action::Fill { target, value, .. } => {
            assert_eq!(value, typed);
            assert_eq!(target.css_path(), "#name");
        }
        other => panic!("not a fill: {other:?}"),
    }
    assert_eq!(recording.steps[0].at_ms, 101);
    assert!(recording.steps[0].only_in.is_empty());
}

#[test]
fn a_typed_password_is_nowhere_but_its_placeholder_is() {
    let mut recorder = started(
        RecorderOptions::default(),
        Some("http://127.0.0.1:8000/form.html"),
    );
    let password = css("#password");
    for (i, c) in PASSWORD.chars().enumerate() {
        let ms = 100 + i as u64 * 40;
        // `@` on a German keyboard is AltGr+Q: Ctrl+Alt in the event.
        let modifiers = if c == '@' { 3 } else { 0 };
        let text = c.to_string();
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms,
            key(&text, modifiers, Some(password.clone())),
        );
        let typed = &PASSWORD[..PASSWORD
            .char_indices()
            .nth(i + 1)
            .map_or(PASSWORD.len(), |(at, _)| at)];
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms + 1,
            input(password.clone(), typed, true),
        );
    }
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        900,
        change(password.clone(), Some(PASSWORD), true),
    );
    feed(
        &mut recorder,
        DESKTOP,
        &[MOBILE],
        950,
        key("Enter", 0, Some(password)),
    );
    let logged = format!("{recorder:?}");
    let recording = recorder.stop().expect("stops");
    let file = to_json(&recording).expect("serialises");
    for (what, text) in [
        ("log", logged),
        ("model", format!("{recording:?}")),
        ("file", file.clone()),
    ] {
        assert!(!text.contains(PASSWORD), "the password is in the {what}");
        assert!(
            !text.contains("Tr0ub"),
            "a part of the password is in the {what}"
        );
    }
    assert!(file.contains("{{secret:password}}"), "{file}");
    let kinds: Vec<&str> = actions(&recording)
        .iter()
        .map(|action| match action {
            Action::Navigate { .. } => "navigate",
            Action::Fill { .. } => "fill",
            Action::Press { key, .. } if key == "Enter" => "enter",
            _ => "other",
        })
        .collect();
    assert_eq!(kinds, ["navigate", "fill", "enter"]);
}

#[test]
fn a_configured_secret_field_is_masked_too() {
    let options = RecorderOptions {
        secret_fields: vec!["#otp".into()],
        ..RecorderOptions::default()
    };
    let mut recorder = started(options, None);
    feed(
        &mut recorder,
        DESKTOP,
        &[],
        10,
        input(css("#otp"), "123456", false),
    );
    let recording = recorder.stop().expect("stops");
    let file = to_json(&recording).expect("serialises");
    assert!(!file.contains("123456"));
    assert!(file.contains("{{secret:otp}}"));
}

#[test]
fn the_burger_click_only_in_the_mobile_view_is_only_in_mobile() {
    let mut recorder = started(RecorderOptions::default(), None);
    recorder.set_leader(Some(MOBILE));
    let burger = role("button", "Menu", "#burger");
    let event = click(burger.clone());
    feed(&mut recorder, MOBILE, &[DESKTOP], 10, event.clone());
    recorder.on_replayed(DESKTOP, &event, &ReplayOutcome::NotVisible);
    // A link both views have.
    let about = click(testid("nav-about", "#mobile-menu > a"));
    feed(&mut recorder, MOBILE, &[DESKTOP], 20, about.clone());
    recorder.on_replayed(DESKTOP, &about, &ReplayOutcome::Done);
    let recording = recorder.stop().expect("stops");
    assert_eq!(recording.steps[0].only_in, ["mobile"]);
    assert_eq!(recording.steps[0].view, "mobile");
    assert!(recording.steps[1].only_in.is_empty());
    let json = serde_json::to_value(&recording.steps[0]).expect("serialises");
    assert_eq!(json["onlyIn"], serde_json::json!(["mobile"]));
}

#[test]
fn a_view_the_hub_did_not_replay_into_is_not_in_only_in() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed(&mut recorder, DESKTOP, &[], 10, click(css("#pricing")));
    let recording = recorder.stop().expect("stops");
    assert_eq!(recording.steps[0].only_in, ["desktop"]);
}

#[test]
fn pause_suppresses_steps_and_resume_records_again() {
    let mut recorder = started(RecorderOptions::default(), Some("http://x.test/a"));
    feed(&mut recorder, DESKTOP, &[MOBILE], 10, click(css("#one")));
    recorder.pause().expect("pauses");
    assert_eq!(recorder.state(), RecorderState::Paused);
    feed(&mut recorder, DESKTOP, &[MOBILE], 20, click(css("#two")));
    recorder.on_navigated(DESKTOP, "http://x.test/b", START + 30);
    recorder
        .resume(Some("http://x.test/b"), START + 40)
        .expect("resumes");
    feed(&mut recorder, DESKTOP, &[MOBILE], 50, click(css("#three")));
    let recording = recorder.stop().expect("stops");
    let paths: Vec<String> = actions(&recording)
        .iter()
        .map(|action| match action {
            Action::Click { target, .. } => target.css_path().to_owned(),
            Action::Navigate { url } => url.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        paths,
        ["http://x.test/a", "#one", "http://x.test/b", "#three"]
    );
}

#[test]
fn a_navigation_after_a_click_waits_for_the_url_a_typed_one_navigates() {
    let mut recorder = started(RecorderOptions::default(), Some("http://x.test/"));
    feed(&mut recorder, DESKTOP, &[MOBILE], 100, click(css("#go")));
    recorder.on_navigated(DESKTOP, "http://x.test/next", START + 600);
    // A follower's navigation is no step.
    recorder.on_navigated(MOBILE, "http://x.test/next", START + 700);
    recorder.on_navigated(DESKTOP, "http://x.test/typed", START + 60_000);
    let recording = recorder.stop().expect("stops");
    assert_eq!(
        actions(&recording),
        [
            &Action::Navigate {
                url: "http://x.test/".into()
            },
            &Action::Click {
                target: Locator {
                    selectors: vec![Selector::Css { path: "#go".into() }],
                    hosts: Vec::new(),
                    frame: None,
                },
                button: None,
                modifiers: 0,
            },
            &Action::WaitForUrl {
                url: "http://x.test/next".into()
            },
            &Action::Navigate {
                url: "http://x.test/typed".into()
            },
        ]
    );
}

/// RB-050-01 review: a key in a password field is decided on before it is masked - a
/// character, Backspace or Delete is never a step (so neither its mask nor the password's
/// length is in the recording); a named key is a step in clear, and Enter still makes the next
/// navigation a `waitForUrl`.
#[test]
fn keys_in_a_password_field_are_no_steps_but_named_keys_are_in_clear() {
    let mut recorder = started(RecorderOptions::default(), Some("http://x.test/login"));
    let password = css("#password");
    for (i, (at, c)) in PASSWORD.char_indices().enumerate() {
        let ms = 100 + i as u64 * 40;
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms,
            secret_key(&c.to_string(), password.clone()),
        );
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms + 1,
            input(password.clone(), &PASSWORD[..at + c.len_utf8()], true),
        );
    }
    for (ms, name) in [
        (800, "Backspace"),
        (810, "Delete"),
        (820, "Tab"),
        (900, "Enter"),
    ] {
        feed(
            &mut recorder,
            DESKTOP,
            &[MOBILE],
            ms,
            secret_key(name, password.clone()),
        );
    }
    recorder.on_navigated(DESKTOP, "http://x.test/home", START + 1_500);
    let recording = recorder.stop().expect("stops");
    let file = to_json(&recording).expect("serialises");
    assert!(!file.contains(crate::forms::MASK), "{file}");
    let kinds: Vec<String> = actions(&recording)
        .iter()
        .map(|action| match action {
            Action::Navigate { .. } => "navigate".to_owned(),
            Action::Fill { .. } => "fill".to_owned(),
            Action::Press { key, .. } => format!("press {key}"),
            Action::WaitForUrl { .. } => "waitForUrl".to_owned(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(
        kinds,
        ["navigate", "fill", "press Tab", "press Enter", "waitForUrl"]
    );
}

/// RB-050-01 review: only a fill the recorder made from a secret field is marked secret; text
/// typed into another field that looks like a placeholder stays text.
#[test]
fn only_a_secret_field_s_fill_is_marked_secret() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed(
        &mut recorder,
        DESKTOP,
        &[],
        10,
        input(css("#password"), PASSWORD, true),
    );
    feed(
        &mut recorder,
        DESKTOP,
        &[],
        20,
        input(css("#note"), "{{secret:admin}}", false),
    );
    let recording = recorder.stop().expect("stops");
    let fills: Vec<(&str, bool)> = actions(&recording)
        .iter()
        .filter_map(|action| match action {
            Action::Fill { value, secret, .. } => Some((value.as_str(), *secret)),
            _ => None,
        })
        .collect();
    assert_eq!(
        fills,
        [("{{secret:password}}", true), ("{{secret:admin}}", false)]
    );
}

/// RB-050-01 review: a token in a URL (an OAuth `code`, an `access_token` in the fragment, a
/// signature, a parameter named in `secretFields`) is recorded masked, the rest in clear.
#[test]
fn tokens_in_urls_are_recorded_masked() {
    let options = RecorderOptions {
        secret_fields: vec!["Session".into()],
        ..RecorderOptions::default()
    };
    let mut recorder = started(
        options,
        Some("http://x.test/cb?code=abc123&state=s1#access_token=tok9&token_type=bearer"),
    );
    recorder.on_navigated(
        DESKTOP,
        "http://x.test/app?session=xyz&page=2",
        START + 60_000,
    );
    let recording = recorder.stop().expect("stops");
    let file = to_json(&recording).expect("serialises");
    for secret in ["abc123", "tok9", "xyz"] {
        assert!(!file.contains(secret), "{secret} in {file}");
    }
    assert_eq!(
        recording.start_url.as_deref(),
        Some("http://x.test/cb?code=********&state=s1#access_token=********&token_type=bearer")
    );
    assert_eq!(
        actions(&recording)[1],
        &Action::Navigate {
            url: "http://x.test/app?session=********&page=2".into()
        }
    );
}

/// RB-094-10: an address's user info (`user:password@`) never reaches a recording.
#[test]
fn the_user_info_of_an_address_is_not_recorded() {
    let mut recorder = started(
        RecorderOptions::default(),
        Some("https://ann:pa55word@x.test/start"),
    );
    recorder.on_navigated(DESKTOP, "https://ann:pa55word@x.test/next", START + 60_000);
    let recording = recorder.stop().expect("stops");
    let file = to_json(&recording).expect("serialises");
    assert!(
        !file.contains("pa55word") && !file.contains("ann:"),
        "{file}"
    );
    assert_eq!(recording.start_url.as_deref(), Some("https://x.test/start"));
}

/// RB-050-01 review: the finished recording can be taken without ending it, so a stop whose
/// save fails loses nothing.
#[test]
fn a_snapshot_leaves_the_recording_running() {
    let mut recorder = started(RecorderOptions::default(), None);
    feed(&mut recorder, DESKTOP, &[], 10, click(css("#one")));
    let snapshot = recorder.snapshot().expect("a recording runs");
    assert_eq!(recorder.state(), RecorderState::Recording);
    assert_eq!(recorder.step_count(), 1);
    feed(&mut recorder, DESKTOP, &[], 20, click(css("#two")));
    let recording = recorder.stop().expect("stops");
    assert_eq!(snapshot.steps.len(), 1);
    assert_eq!(snapshot.steps[0].only_in, ["desktop"]);
    assert_eq!(recording.steps.len(), 2);
    assert_eq!(recording.steps[0], snapshot.steps[0]);
    assert_eq!(
        Recorder::new(RecorderOptions::default()).snapshot(),
        Err(RecorderError::NotRecording)
    );
}

#[test]
fn start_and_stop_say_when_they_cannot() {
    let mut recorder = Recorder::new(RecorderOptions::default());
    assert_eq!(recorder.stop(), Err(RecorderError::NotRecording));
    assert_eq!(recorder.pause(), Err(RecorderError::NotRecording));
    let mut recorder = started(RecorderOptions::default(), None);
    let again = recorder.start(StartInfo {
        name: "x".into(),
        start_url: None,
        environment: None,
        app_version: String::new(),
        views: Vec::new(),
        leader: None,
        now_ms: 0,
    });
    assert_eq!(again, Err(RecorderError::AlreadyRecording));
    assert_eq!(recorder.name(), Some("login"));
}

#[test]
fn the_recording_carries_its_metadata() {
    let mut recorder = started(RecorderOptions::default(), Some("http://x.test/"));
    let recording = recorder.stop().expect("stops");
    assert_eq!(recording.version, RECORDING_VERSION);
    assert_eq!(recording.schema, None);
    assert_eq!(recording.started_at_ms, START);
    assert_eq!(recording.environment.as_deref(), Some("local"));
    let ids: Vec<&str> = recording
        .breakpoints
        .iter()
        .map(|bp| bp.id.as_str())
        .collect();
    assert_eq!(ids, ["desktop", "mobile"]);
    assert_eq!(recorder.state(), RecorderState::Idle);
}

#[test]
fn events_of_a_stopped_recorder_are_ignored() {
    let mut recorder = Recorder::new(RecorderOptions::default());
    feed(&mut recorder, DESKTOP, &[], 10, click(css("#x")));
    recorder.on_navigated(DESKTOP, "http://x.test/", START);
    assert_eq!(recorder.step_count(), 0);
}

fn heading() -> Locator {
    Locator {
        selectors: vec![Selector::Css { path: "h1".into() }],
        hosts: Vec::new(),
        frame: None,
    }
}

#[test]
fn a_check_from_the_page_menu_is_a_step_of_the_views_that_have_the_element() {
    // RB-090-60: "Check: element visible" on the mobile view's heading, found only there.
    let mut recorder = started(RecorderOptions::default(), None);
    let check = Action::Assert {
        assertion: crate::recorder::model::Assertion::Visible { target: heading() },
    };
    let id = recorder
        .add_check(MOBILE, check.clone(), &[MOBILE], START + 500)
        .expect("recording");
    let wait = Action::WaitFor { target: heading() };
    recorder
        .add_check(MOBILE, wait.clone(), &[], START + 600)
        .expect("recording");
    let recording = recorder.stop().expect("stops");
    assert_eq!(actions(&recording), vec![&check, &wait]);
    assert_eq!(recording.steps[0].id, id);
    assert_eq!(recording.steps[0].view, "mobile");
    assert_eq!(recording.steps[0].at_ms, 500);
    assert_eq!(recording.steps[0].only_in, vec!["mobile".to_owned()]);
    // No views named: every view of the recording, so no `onlyIn`.
    assert!(recording.steps[1].only_in.is_empty());
}

#[test]
fn a_check_is_taken_while_paused_and_refused_without_a_recording() {
    let mut recorder = started(RecorderOptions::default(), None);
    recorder.pause().expect("pauses");
    let wait = Action::WaitFor { target: heading() };
    assert!(
        recorder
            .add_check(DESKTOP, wait.clone(), &[], START)
            .is_ok()
    );
    assert_eq!(recorder.step_count(), 1);
    recorder.stop().expect("stops");
    assert_eq!(
        recorder.add_check(DESKTOP, wait, &[], START),
        Err(RecorderError::NotRecording)
    );
}
