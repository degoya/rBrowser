use rb_engine::ViewId;
use rb_visual::{Status, Subject};
use serde_json::{Value, json};

use super::*;

fn view(id: u32, name: &str) -> ViewTarget {
    ViewTarget {
        id: ViewId(id),
        label: String::new(),
        name: name.to_owned(),
        url: "http://localhost:5173/".to_owned(),
        engine: "chromium".to_owned(),
        viewport: rb_visual::Viewport::default(),
    }
}

#[test]
fn the_request_names_only_the_plugin_and_carries_the_comparisons() {
    let differs = Comparison {
        subject: Subject {
            view: 2,
            breakpoint: "desktop".to_owned(),
            ..Subject::default()
        },
        status: Status::Differs,
        ratio: 0.25,
        ..Comparison::default()
    };
    let request = request(
        "run-5".to_owned(),
        5,
        Target::View(ViewId(2)),
        &[view(2, "desktop")],
        &[differs],
        true,
    );
    assert_eq!(request.plugins, Some(vec!["visual-regression".to_owned()]));
    assert_eq!(request.target, CheckTarget::View(2));
    assert_eq!(request.views.len(), 1);
    assert_eq!(request.views[0].breakpoint.as_deref(), Some("desktop"));
    let args: Value = serde_json::from_str(&request.args).expect("json");
    assert_eq!(args["visual"]["failOnMetaMismatch"], json!(true));
    assert_eq!(args["visual"]["comparisons"][0]["status"], json!("differs"));
    assert_eq!(args["visual"]["comparisons"][0]["view"], json!(2));
}
