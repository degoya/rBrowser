use std::sync::{Arc, Mutex};

use rb_ipc::ExtensionPromptKind;

use super::*;

fn request(id: &str) -> PromptRequest {
    PromptRequest {
        kind: ExtensionPromptKind::Install,
        id: id.to_owned(),
        name: "Vue.js devtools".to_owned(),
        version: "7.6.4".to_owned(),
        current_version: None,
    }
}

fn recorder() -> (Arc<Mutex<Vec<bool>>>, Decided) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();
    (
        seen,
        Box::new(move |accepted| sink.lock().expect("lock").push(accepted)),
    )
}

#[test]
fn a_prompt_gets_a_fresh_id_and_carries_the_request() {
    let mut prompts = Prompts::default();
    let (_, done) = recorder();
    let first = prompts.ask(request("a".repeat(32).as_str()), done);
    let (_, done) = recorder();
    let second = prompts.ask(request("b".repeat(32).as_str()), done);
    assert_ne!(first.prompt_id, second.prompt_id);
    assert_eq!(first.kind, ExtensionPromptKind::Install);
    assert_eq!(first.name, "Vue.js devtools");
    assert_eq!(first.version, "7.6.4");
    assert_eq!(second.id, "b".repeat(32));
}

#[test]
fn the_decision_reaches_the_asker_once() {
    let mut prompts = Prompts::default();
    let (seen, done) = recorder();
    let prompt = prompts.ask(request("a".repeat(32).as_str()), done);
    let decided = prompts.decide(prompt.prompt_id, true).expect("open");
    decided(true);
    assert_eq!(*seen.lock().expect("lock"), [true]);
    assert!(
        prompts.decide(prompt.prompt_id, true).is_none(),
        "answered already"
    );
    assert!(prompts.decide(99, true).is_none(), "never asked");
}

#[test]
fn a_forgotten_prompt_answers_no() {
    let mut prompts = Prompts::default();
    let (seen, done) = recorder();
    let prompt = prompts.ask(request("a".repeat(32).as_str()), done);
    prompts.forget(prompt.prompt_id);
    assert_eq!(*seen.lock().expect("lock"), [false]);
    assert!(prompts.decide(prompt.prompt_id, true).is_none());
}

#[test]
fn a_second_prompt_for_the_same_extension_replaces_the_first() {
    let mut prompts = Prompts::default();
    let (first_seen, done) = recorder();
    let first = prompts.ask(request("a".repeat(32).as_str()), done);
    let (_, done) = recorder();
    let second = prompts.ask(request("a".repeat(32).as_str()), done);
    assert_eq!(
        *first_seen.lock().expect("lock"),
        [false],
        "the first is refused"
    );
    assert!(prompts.decide(first.prompt_id, true).is_none());
    assert!(prompts.decide(second.prompt_id, true).is_some());
}

#[test]
fn more_open_prompts_than_the_limit_forget_the_oldest() {
    let mut prompts = Prompts::default();
    let (oldest, done) = recorder();
    let first = prompts.ask(request("a".repeat(32).as_str()), done);
    for index in 0..MAX_OPEN_PROMPTS {
        let letter = char::from(b'b' + u8::try_from(index).expect("fits"));
        let id = format!("{}{letter}", "b".repeat(31));
        let (_, done) = recorder();
        prompts.ask(request(&id), done);
    }
    assert_eq!(*oldest.lock().expect("lock"), [false]);
    assert!(prompts.decide(first.prompt_id, true).is_none());
}
