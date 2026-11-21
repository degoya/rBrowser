use serde_json::json;

use super::*;
use crate::fake::{SweepHost, overflow};

#[test]
fn the_script_is_the_detector_and_one_call_of_it_with_the_width_and_kinds() {
    let kinds = vec!["overflow".to_owned(), "overlap".to_owned()];
    let script = script(720, &kinds, true);
    assert!(script.starts_with(DETECT));
    assert!(DETECT.contains("WidthSweepDetect"), "the bundle's global");
    let call = script.trim_start_matches(DETECT);
    assert_eq!(
        call,
        format!(
            "\n;WidthSweepDetect.probe({})",
            json!({ "width": 720, "kinds": ["overflow", "overlap"], "reset": true })
        )
    );
}

#[test]
fn a_probe_reads_the_pages_answer() {
    let mut host = SweepHost::with_views(&[1]);
    host.page = Box::new(|width| {
        if width == 720 {
            vec![overflow(".hero img")]
        } else {
            Vec::new()
        }
    });
    host.width = Some(720);
    let answer = probe(&mut host, 1, 720, &["overflow".to_owned()], false).expect("an answer");
    assert_eq!(answer.width, 720);
    assert!(!answer.escape);
    assert_eq!(answer.issues, vec![overflow(".hero img")]);
}

#[test]
fn an_answer_that_is_no_probe_is_bad_data() {
    for text in ["null", "[]", r#"{"issues":[]}"#, "not json"] {
        let failure = parse(text).expect_err(text);
        assert_eq!(failure.code(), Some(PAGE_FAILED_KEY), "{text}");
    }
    let answer = parse(r#"{"width":400,"issues":[{"kind":"overlap","selector":"a","other":"b"}]}"#)
        .expect("a probe");
    assert_eq!(answer.issues[0].other.as_deref(), Some("b"));
    assert!(!answer.escape);
}
