use std::time::{Duration, Instant};

use rb_engine::ViewId;

use super::*;

fn key(origin: &str, realm: &str, proxy: bool) -> PromptKey {
    PromptKey {
        origin: origin.to_owned(),
        realm: realm.to_owned(),
        proxy,
    }
}

fn site() -> PromptKey {
    key("https://other.example.test:443", "Members", false)
}

/// A waiter is a number here: which request got what.
fn waiting(key: PromptKey, view: u32, browser: i32, waiter: u32) -> Waiting<u32> {
    Waiting {
        key,
        view: ViewId(view),
        browser,
        environment: None,
        stored_refused: false,
        insecure: false,
        waiter,
    }
}

fn shown(asked: Asked<u32>) -> (AuthPrompt, bool) {
    match asked {
        Asked::Show { prompt, new } => (prompt, new),
        Asked::Cancel(waiter) | Asked::Gone(waiter) => panic!("cancelled {waiter}"),
    }
}

#[test]
fn views_asked_at_once_wait_on_one_question_and_share_its_answer() {
    let mut prompts = Prompts::new();
    let (first, new) = shown(prompts.ask(waiting(site(), 1, 11, 100)));
    assert!(new);
    assert_eq!(first.views, vec![ViewId(1)]);
    assert_eq!(first.origin, "https://other.example.test:443");
    assert_eq!(first.realm, "Members");
    assert!(!first.refused && !first.proxy);
    let (second, new) = shown(prompts.ask(waiting(site(), 2, 12, 200)));
    assert!(!new, "the same question, not a second one");
    assert_eq!(second.id, first.id);
    assert_eq!(second.views, vec![ViewId(1), ViewId(2)]);
    // A second request of view 1 joins too, and the view is named once.
    let (third, _) = shown(prompts.ask(waiting(site(), 1, 11, 101)));
    assert_eq!(third.views, vec![ViewId(1), ViewId(2)]);
    let answered = prompts.sign_in(first.id).expect("open");
    assert_eq!(answered.waiters, vec![100, 200, 101]);
    assert_eq!(answered.key, site());
    assert!(prompts.sign_in(first.id).is_none(), "answered once");
    assert!(prompts.is_empty());
}

#[test]
fn another_realm_a_proxy_or_another_origin_is_a_question_of_its_own() {
    let mut prompts = Prompts::new();
    let (first, _) = shown(prompts.ask(waiting(site(), 1, 11, 1)));
    for other in [
        key("https://other.example.test:443", "Admin", false),
        key("https://other.example.test:443", "Members", true),
        key("https://other.example.test:8443", "Members", false),
    ] {
        let (prompt, new) = shown(prompts.ask(waiting(other, 1, 11, 2)));
        assert!(new);
        assert_ne!(prompt.id, first.id);
    }
}

#[test]
fn a_refused_answer_asks_again_saying_so_and_stops_after_the_limit() {
    let mut prompts = Prompts::new();
    for round in 0..MAX_TRIES {
        let (prompt, new) = shown(prompts.ask(waiting(site(), 1, 11, round)));
        assert!(new);
        assert_eq!(prompt.refused, round > 0, "round {round}");
        assert_eq!(
            prompts.sign_in(prompt.id).expect("open").waiters,
            vec![round]
        );
    }
    // The server refused every answer: CEF cancels, the 401 page shows.
    assert!(matches!(
        prompts.ask(waiting(site(), 1, 11, 9)),
        Asked::Cancel(9)
    ));
    // Another view has its own count.
    assert!(!shown(prompts.ask(waiting(site(), 2, 12, 10))).0.refused);
    // A credential set or removed starts over.
    prompts.reset_tries();
    let (_, open) = prompts.drop_browser(12);
    assert_eq!(open.len(), 1, "view 2's question withdrawn");
    assert!(!shown(prompts.ask(waiting(site(), 1, 11, 11))).0.refused);
}

#[test]
fn a_stored_credential_the_server_refused_is_said_at_once() {
    let mut prompts = Prompts::new();
    let mut asked = waiting(site(), 1, 11, 1);
    asked.stored_refused = true;
    asked.environment = Some("development".to_owned());
    let (prompt, _) = shown(prompts.ask(asked));
    assert!(prompt.refused);
    assert_eq!(prompt.environment.as_deref(), Some("development"));
    assert_eq!(
        prompts.environment(prompt.id),
        Some(Some("development".to_owned()))
    );
    assert_eq!(prompts.environment(prompt.id + 1), None);
}

#[test]
fn cancel_gives_back_every_waiter() {
    let mut prompts = Prompts::new();
    let (prompt, _) = shown(prompts.ask(waiting(site(), 1, 11, 1)));
    let _ = prompts.ask(waiting(site(), 2, 12, 2));
    assert_eq!(prompts.cancel(prompt.id), Some(vec![1, 2]));
    assert_eq!(prompts.cancel(prompt.id), None);
    // Cancel is no refusal: the next question of the view does not say so.
    assert!(!shown(prompts.ask(waiting(site(), 1, 11, 3))).0.refused);
}

#[test]
fn a_closed_view_drops_its_requests_and_a_question_nobody_waits_on() {
    let mut prompts = Prompts::new();
    let (shared, _) = shown(prompts.ask(waiting(site(), 1, 11, 1)));
    let _ = prompts.ask(waiting(site(), 2, 12, 2));
    let (alone, _) = shown(prompts.ask(waiting(
        key("https://solo.example.test:443", "", false),
        2,
        12,
        3,
    )));
    let (dropped, changes) = prompts.drop_browser(12);
    assert_eq!(dropped, vec![2, 3]);
    assert_eq!(changes.len(), 2, "{changes:?}");
    assert!(changes.iter().any(|change| matches!(
        change,
        Change::Show(prompt) if prompt.id == shared.id && prompt.views == vec![ViewId(1)]
    )));
    assert!(changes.contains(&Change::Closed(alone.id)));
    assert_eq!(prompts.drop_browser(99), (Vec::new(), Vec::new()));
    assert_eq!(
        prompts.sign_in(shared.id).expect("still open").waiters,
        vec![1]
    );
}

/// Review w35: a view whose request comes after it closed (its store read still ran) is never
/// asked - no question for a dead browser.
#[test]
fn a_closed_view_is_never_asked_again() {
    let mut prompts = Prompts::new();
    let _ = prompts.drop_browser(11);
    assert!(matches!(
        prompts.ask(waiting(site(), 1, 11, 7)),
        Asked::Gone(7)
    ));
    assert!(prompts.is_empty());
    assert!(matches!(
        prompts.ask(waiting(site(), 2, 12, 8)),
        Asked::Show { new: true, .. }
    ));
}

/// Review w35: only refusals count. The same challenge soon after an answer is a refusal; a
/// challenge after [`REFUSAL_WINDOW`] means the answer before was accepted - a fresh question,
/// never marked refused, and the count starts over.
#[test]
fn an_accepted_answer_is_no_refusal() {
    let mut prompts = Prompts::new();
    let start = Instant::now();
    let later = start + REFUSAL_WINDOW + Duration::from_secs(1);
    let (first, _) = shown(prompts.ask_at(waiting(site(), 1, 11, 1), start));
    let _ = prompts.sign_in_at(first.id, start);
    // Accepted: the next challenge comes long after.
    let (fresh, _) = shown(prompts.ask_at(waiting(site(), 1, 11, 2), later));
    assert!(!fresh.refused);
    let _ = prompts.sign_in_at(fresh.id, later);
    // Refused twice in a row, then accepted: never cancelled, the count starts over.
    let mut now = later;
    for round in 0..MAX_TRIES - 1 {
        now += Duration::from_secs(1);
        let (again, _) = shown(prompts.ask_at(waiting(site(), 1, 11, 10 + round), now));
        assert!(again.refused, "round {round}");
        let _ = prompts.sign_in_at(again.id, now);
    }
    let much_later = now + REFUSAL_WINDOW * 2;
    let (next, _) = shown(prompts.ask_at(waiting(site(), 1, 11, 20), much_later));
    assert!(!next.refused);
}

#[test]
fn a_plain_http_question_says_it_is_unencrypted() {
    let mut prompts = Prompts::new();
    let mut asked = waiting(key("http://plain.example.test:80", "", false), 1, 11, 1);
    asked.insecure = true;
    let (prompt, _) = shown(prompts.ask(asked));
    assert!(prompt.insecure);
    assert_eq!(
        prompts.origin(prompt.id).as_deref(),
        Some("http://plain.example.test:80")
    );
    assert!(!shown(prompts.ask(waiting(site(), 1, 11, 2))).0.insecure);
}
