use std::path::PathBuf;
use std::time::{Duration, Instant};

use rb_engine::ViewId;
use serde_json::json;

use super::add_design::{Added, Refusal};
use super::*;

#[test]
fn only_an_offered_action_of_the_plugin_itself_runs() {
    let add = json!({ "action": "add_design" });
    assert_eq!(offered("image-overlay", &add), Some("add_design"));
    // Another plugin cannot ask for it, nor the plugin for another action.
    assert_eq!(offered("color-picker", &add), None);
    assert_eq!(
        offered("image-overlay", &json!({ "action": "grant_trust" })),
        None
    );
    assert_eq!(offered("image-overlay", &json!({ "action": 1 })), None);
    assert_eq!(offered("image-overlay", &json!("add_design")), None);
}

#[test]
fn only_the_form_autofill_reports_a_fill_and_needs_no_gesture_for_it() {
    // RB-090-78: the fields a fill set, for a running recording.
    let record = json!({ "action": "record_fill", "view": 1, "fields": [] });
    assert_eq!(offered("form-autofill", &record), Some("record_fill"));
    assert_eq!(offered("image-overlay", &record), None);
    assert!(gestureless("form-autofill", &record));
    assert!(!gestureless("image-overlay", &record));
    assert!(!gestureless(
        "form-autofill",
        &json!({ "action": "add_design" })
    ));
    // Adding a design opens a dialog: a person's command only.
    assert!(!gestureless(
        "image-overlay",
        &json!({ "action": "add_design" })
    ));
}

#[test]
fn a_design_is_added_only_to_an_open_trusted_project() {
    let root = PathBuf::from("/work/site");
    assert_eq!(admit(None, true), Err(NO_PROJECT_KEY));
    assert_eq!(admit(None, false), Err(NO_PROJECT_KEY));
    assert_eq!(admit(Some(root.clone()), false), Err(UNTRUSTED_KEY));
    assert_eq!(admit(Some(root.clone()), true), Ok(root));
}

#[test]
fn one_file_dialog_at_a_time() {
    let first = PickGuard::take().expect("the first pick");
    assert!(PickGuard::take().is_none(), "a second dialog waits");
    drop(first);
    assert!(
        PickGuard::take().is_some(),
        "free again once the first ended"
    );
}

fn target(id: &str, label: &str) -> Target {
    Target {
        id: id.to_owned(),
        label: label.to_owned(),
    }
}

#[test]
fn the_design_goes_to_the_active_views_breakpoint() {
    let views = vec![
        (ViewId(1), Some("mobile".to_owned()), "Mobile".to_owned()),
        (ViewId(2), Some("desktop".to_owned()), "Desktop".to_owned()),
        (ViewId(3), None, "500 x 800".to_owned()),
    ];
    assert_eq!(
        active_target(views.clone(), Some(ViewId(2))),
        Some(target("desktop", "Desktop"))
    );
    // A free size, no active view, or an id the overlay does not take: no breakpoint.
    assert_eq!(active_target(views.clone(), Some(ViewId(3))), None);
    assert_eq!(active_target(views.clone(), None), None);
    let odd = vec![(ViewId(4), Some("a b".to_owned()), "Odd".to_owned())];
    assert_eq!(active_target(odd, Some(ViewId(4))), None);
}

#[test]
fn the_overlay_is_switched_on_with_the_design_for_the_breakpoint() {
    assert_eq!(
        set_args(Some("mobile"), "home (2)-1440.png"),
        json!({ "enabled": true, "designs": { "mobile": "designs/home (2)-1440.png" } })
    );
    assert_eq!(set_args(None, "home.png"), json!({ "enabled": true }));
}

fn args(notice: &Notice) -> Vec<(&str, &str)> {
    notice
        .args
        .iter()
        .map(|(name, value)| (*name, value.as_str()))
        .collect()
}

#[test]
fn every_outcome_has_its_notice() {
    let added = Added {
        name: "home.png".into(),
        original: "home.png".into(),
        reused: false,
    };
    let shown = added_notice(&added, Some(&target("mobile", "Mobile")));
    assert_eq!(shown.level, NotificationLevel::Success);
    assert_eq!(shown.key, ADDED_KEY);
    assert_eq!(
        args(&shown),
        [("name", "home.png"), ("breakpoint", "Mobile")]
    );
    let nearest = added_notice(&added, None);
    assert_eq!(nearest.key, ADDED_NEAREST_KEY);
    let renamed = Added {
        name: "home (2).png".into(),
        original: "home.png".into(),
        reused: false,
    };
    let named = added_notice(&renamed, Some(&target("mobile", "Mobile")));
    assert_eq!(named.key, ADDED_AS_KEY);
    assert_eq!(
        args(&named),
        [
            ("name", "home (2).png"),
            ("original", "home.png"),
            ("breakpoint", "Mobile")
        ]
    );
    let failed = not_shown_notice(&added, "busy".into());
    assert_eq!(failed.level, NotificationLevel::Error);
    assert_eq!(args(&failed), [("name", "home.png"), ("error", "busy")]);

    let cases = [
        (Refusal::BadType, BAD_TYPE_KEY),
        (Refusal::TooLarge, TOO_LARGE_KEY),
        (Refusal::TooManyPixels, TOO_MANY_PIXELS_KEY),
        (Refusal::Unreadable, UNREADABLE_KEY),
        (Refusal::NotWritten, NOT_WRITTEN_KEY),
    ];
    for (refusal, key) in cases {
        let notice = refused_notice("x.gif", refusal);
        assert_eq!(notice.key, key);
        assert_eq!(notice.level, NotificationLevel::Error);
        assert_eq!(args(&notice), [("name", "x.gif")]);
        assert!(notice.failed());
    }
    assert!(!shown.failed());
}

#[test]
fn a_notice_reaches_the_status_bar_and_the_panel_with_the_same_words() {
    let notice = refused_notice("x.gif", Refusal::BadType);
    let notification = notice.notification();
    assert_eq!(notification.level, NotificationLevel::Error);
    assert_eq!(notification.message_key, BAD_TYPE_KEY);
    assert_eq!(
        notification.args.get("name").map(String::as_str),
        Some("x.gif")
    );
    let event = notice.panel_event("No.");
    assert_eq!(event["event"]["plugin"], json!("image-overlay"));
    assert_eq!(
        event["event"]["message"]["hostNotice"],
        json!({ "text": "No.", "failed": true })
    );
}

#[test]
fn a_design_already_in_the_project_is_said_to_be_there() {
    let there = Added {
        name: "home.png".into(),
        original: "home.png".into(),
        reused: true,
    };
    let shown = added_notice(&there, Some(&target("mobile", "Mobile")));
    assert_eq!(shown.key, ALREADY_KEY);
    assert_eq!(shown.level, NotificationLevel::Success);
    assert_eq!(
        args(&shown),
        [("name", "home.png"), ("breakpoint", "Mobile")]
    );
    let nearest = added_notice(&there, None);
    assert_eq!(nearest.key, ALREADY_NEAREST_KEY);
    assert_eq!(args(&nearest), [("name", "home.png")]);
}

#[test]
fn a_cancelled_dialog_says_so_and_ends_the_panels_wait() {
    let notice = cancelled_notice();
    assert_eq!(notice.key, CANCELLED_KEY);
    assert_eq!(notice.level, NotificationLevel::Info);
    assert!(!notice.failed());
    // The panel's "Adding" line ends with the same sentence.
    let event = notice.panel_event("No design added.");
    assert_eq!(
        event["event"]["message"]["hostNotice"],
        json!({ "text": "No design added.", "failed": false })
    );
}

#[test]
fn a_cancelled_dialog_is_not_opened_again_at_once() {
    let start = Instant::now();
    let mut cooldown = Cooldown::default();
    assert!(!cooldown.cooling(start), "nothing cancelled yet");
    cooldown.cancelled(start);
    assert!(cooldown.cooling(start + Duration::from_millis(500)));
    assert!(cooldown.cooling(start + COOLDOWN - Duration::from_millis(1)));
    assert!(!cooldown.cooling(start + COOLDOWN));
    let notice = cooling_notice();
    assert_eq!(notice.key, WAIT_KEY);
    assert_eq!(notice.level, NotificationLevel::Warning);
}
