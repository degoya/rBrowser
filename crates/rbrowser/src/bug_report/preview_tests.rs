use std::sync::{Arc, Mutex};

use serde_json::json;

use super::*;

fn draft(folder: &str) -> Draft {
    Draft {
        folder: folder.to_owned(),
        markdown: "# Bug report\n".to_owned(),
        images: 2,
        annotate: false,
    }
}

/// The answers a preview's asker heard, in order.
fn recorder() -> (Arc<Mutex<Vec<bool>>>, impl Fn() -> Decided) {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&heard);
    let make = move || -> Decided {
        let sink = Arc::clone(&sink);
        Box::new(move |save| sink.lock().expect("the test's lock").push(save))
    };
    (heard, make)
}

#[test]
fn a_preview_waits_until_the_person_decides() {
    let (heard, done) = recorder();
    let mut previews = Previews::default();
    let shown = previews.ask(draft("/p/r"), done());
    assert_eq!(shown.preview_id, 1);
    assert_eq!(shown.folder, "/p/r");
    assert_eq!(shown.markdown, "# Bug report\n");
    assert_eq!(shown.images, 2);
    assert_eq!(previews.current(), Some(shown.clone()));
    assert!(
        heard.lock().expect("lock").is_empty(),
        "nothing decided yet"
    );
    let decided = previews.decide(shown.preview_id).expect("the open preview");
    decided(true);
    assert_eq!(*heard.lock().expect("lock"), [true]);
    assert_eq!(previews.current(), None);
    assert!(previews.decide(shown.preview_id).is_none(), "decided once");
}

#[test]
fn a_new_preview_cancels_the_one_before() {
    let (heard, done) = recorder();
    let mut previews = Previews::default();
    let first = previews.ask(draft("/p/a"), done());
    let second = previews.ask(draft("/p/b"), done());
    assert_ne!(first.preview_id, second.preview_id);
    assert_eq!(
        *heard.lock().expect("lock"),
        [false],
        "the first is dropped"
    );
    assert!(previews.decide(first.preview_id).is_none());
    assert_eq!(
        previews.current().map(|preview| preview.folder),
        Some("/p/b".to_owned())
    );
}

#[test]
fn a_forgotten_preview_is_cancelled_and_a_late_answer_finds_nothing() {
    let (heard, done) = recorder();
    let mut previews = Previews::default();
    let shown = previews.ask(draft("/p/a"), done());
    previews.forget(Some(shown.preview_id + 1));
    assert!(previews.current().is_some(), "another id forgets nothing");
    previews.forget(Some(shown.preview_id));
    assert_eq!(*heard.lock().expect("lock"), [false]);
    assert!(previews.decide(shown.preview_id).is_none());
    let again = previews.ask(draft("/p/b"), done());
    previews.forget(None);
    assert_eq!(*heard.lock().expect("lock"), [false, false]);
    assert!(previews.decide(again.preview_id).is_none());
}

#[test]
fn the_saved_reports_keep_the_newest_texts_by_name() {
    let mut saved = Saved::default();
    for n in 0..=MAX_SAVED {
        saved.remember(format!("r{n}"), format!("text {n}"));
    }
    assert_eq!(saved.text_of("r0"), None, "the oldest went");
    assert_eq!(saved.text_of("r1"), Some("text 1"));
    assert_eq!(
        saved.text_of(&format!("r{MAX_SAVED}")),
        Some(format!("text {MAX_SAVED}").as_str())
    );
    saved.remember("r1".to_owned(), "new".to_owned());
    assert_eq!(saved.text_of("r1"), Some("new"));
    assert_eq!(saved.text_of("../r1"), None);
    assert_eq!(saved.text_of(""), None);
}

#[test]
fn only_a_decision_is_kept_to_the_pages_window() {
    let request =
        |request: serde_json::Value| json!({ "type": "bugReport", "request": request }).to_string();
    assert!(is_decision_request(&request(
        json!({ "op": "decide", "previewId": 1, "save": true })
    )));
    assert!(!is_decision_request(&request(
        json!({ "op": "getPreview" })
    )));
    assert!(!is_decision_request(&request(
        json!({ "op": "getMarkdown", "report": "r" })
    )));
    assert!(!is_decision_request(r#"{"type":"getState"}"#));
    assert!(!is_decision_request("not json"));
}

#[test]
fn the_page_route_is_the_shells() {
    assert_eq!(ROUTE, "bug-report");
    assert!(crate::shell::page_url(ROUTE).ends_with("#/bug-report"));
}
