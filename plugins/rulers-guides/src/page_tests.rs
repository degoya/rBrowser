use super::*;
use crate::store::{STATE_FILE, STATE_PATH};
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use serde_json::{Value, json};

fn view(id: u32, breakpoint: &str, width: u32) -> ViewInfo {
    ViewInfo {
        id,
        label: breakpoint.to_owned(),
        breakpoint: breakpoint.to_owned(),
        engine: "chromium".to_owned(),
        width,
        height: 800,
        dpr: 1.0,
        url: "https://example.test/".to_owned(),
        title: String::new(),
    }
}

fn host() -> MemoryHost {
    let mut host = MemoryHost::new()
        .grant(Capability::Views)
        .grant(Capability::ProjectRead)
        .grant(Capability::OutWrite);
    host.views = vec![view(1, "mobile", 375), view(2, "desktop", 1440)];
    host
}

fn ask(host: &mut MemoryHost, view: u32, message: Value) -> Result<Value, Failure> {
    let args = json!({ "view": view, "message": message }).to_string();
    handle(host, &args).map(|text| serde_json::from_str(&text).expect("JSON answer"))
}

/// The state file written by the last change, made readable as the next call would find it.
fn persist(host: MemoryHost) -> MemoryHost {
    match host.out.get(STATE_FILE).cloned() {
        Some(bytes) => host.with_file(STATE_PATH, &bytes),
        None => host,
    }
}

fn geometry(width: f64) -> Value {
    json!({ "clientWidth": width, "innerWidth": width })
}

#[test]
fn init_answers_the_state_the_prefs_and_the_views_drawing() {
    let mut host = host();
    host.settings.insert(
        "columns".to_owned(),
        r#"{"mobile":{"columns":4,"gutter":16,"margin":16}}"#.to_owned(),
    );
    host.settings.insert("snapPx".to_owned(), "4".to_owned());
    let answer = ask(
        &mut host,
        1,
        json!({"op": "init", "geometry": geometry(375.0)}),
    )
    .expect("init");
    assert_eq!(answer["prefs"], json!({"unit": "px", "snapPx": 4}));
    assert_eq!(answer["layout"]["breakpoint"], "mobile");
    assert_eq!(answer["layout"]["columns"]["columns"][1]["x"], 105.75);
    assert_eq!(answer["state"]["visible"]["rulers"], false);
    assert_eq!(answer["overrides"], json!({"visible": {}}));
    // The desktop view has no setup: no columns.
    let desktop = ask(
        &mut host,
        2,
        json!({"op": "layout", "geometry": geometry(1440.0)}),
    )
    .expect("layout");
    assert_eq!(desktop["layout"]["columns"], Value::Null);
    assert!(desktop.get("state").is_none());
}

#[test]
fn a_guide_dragged_in_one_view_is_posted_to_every_view_and_kept() {
    let mut host = host();
    let answer = ask(
        &mut host,
        1,
        json!({"op": "guide", "action": "add", "axis": "y", "pos": 203.0, "snap": true, "geometry": geometry(375.0)}),
    )
    .expect("added");
    assert_eq!(answer["saved"], true);
    assert_eq!(
        answer["state"]["guides"],
        json!([{"id": 1, "axis": "y", "unit": "px", "value": 200.0}])
    );
    let posts = answer["$post"].as_array().expect("posts");
    assert_eq!(posts[0]["target"], "all");
    assert_eq!(posts[0]["message"]["state"], answer["state"]);
    assert_eq!(
        posts[0]["message"]["overrides"]["guides"],
        answer["state"]["guides"]
    );
    assert_eq!(posts[1]["target"], "panel");

    // The other view, after a restart, draws it from the file.
    let mut later = persist(host);
    let drawn = ask(
        &mut later,
        2,
        json!({"op": "layout", "geometry": geometry(1440.0)}),
    )
    .expect("layout");
    assert_eq!(
        drawn["layout"]["guides"],
        json!([{"id": 1, "axis": "y", "pos": 200.0}])
    );
}

#[test]
fn a_vertical_guide_takes_the_unit_of_the_setting() {
    let mut host = host();
    host.settings
        .insert("unit".to_owned(), "\"percent\"".to_owned());
    let answer = ask(
        &mut host,
        1,
        json!({"op": "guide", "action": "add", "axis": "x", "pos": 187.5, "geometry": geometry(375.0)}),
    )
    .expect("added");
    assert_eq!(answer["state"]["guides"][0]["unit"], "percent");
    assert_eq!(answer["state"]["guides"][0]["value"], 50.0);
    // In the desktop view the same guide lies in its middle.
    let mut later = persist(host);
    let drawn = ask(
        &mut later,
        2,
        json!({"op": "layout", "geometry": geometry(1440.0)}),
    )
    .expect("layout");
    assert_eq!(drawn["layout"]["guides"][0]["pos"], 720.0);
}

#[test]
fn moving_keeps_the_unit_and_removing_drops_the_guide() {
    let mut host = host();
    host.settings.insert(
        "guides".to_owned(),
        r#"[{"id":7,"axis":"x","unit":"px-right","value":16}]"#.to_owned(),
    );
    let moved = ask(
        &mut host,
        1,
        json!({"op": "guide", "action": "move", "id": 7, "pos": 335.0, "geometry": geometry(375.0)}),
    )
    .expect("moved");
    assert_eq!(
        moved["state"]["guides"],
        json!([{"id": 7, "axis": "x", "unit": "px-right", "value": 40.0}])
    );
    let mut host = persist(host);
    let removed = ask(
        &mut host,
        1,
        json!({"op": "guide", "action": "remove", "id": 7}),
    )
    .expect("removed");
    assert_eq!(removed["state"]["guides"], json!([]));
    assert_eq!(
        removed["$post"][0]["message"]["overrides"]["guides"],
        json!([])
    );
}

#[test]
fn without_a_project_the_pages_copy_carries_the_session() {
    let mut host = MemoryHost::new().grant(Capability::Views);
    host.views = vec![view(1, "mobile", 375)];
    let copy = json!({"guides": [{"id": 1, "axis": "y", "unit": "px", "value": 10}]});
    let answer = ask(
        &mut host,
        1,
        json!({"op": "guide", "action": "add", "axis": "y", "pos": 20.0, "geometry": geometry(375.0), "overrides": copy}),
    )
    .expect("added");
    assert_eq!(answer["saved"], false);
    assert_eq!(answer["state"]["guides"].as_array().map(Vec::len), Some(2));
}

#[test]
fn malformed_requests_are_refused_with_the_reason() {
    let mut host = host();
    let code = |result: Result<Value, Failure>| {
        let failure = result.expect_err("refused");
        (
            failure.code.unwrap_or_default(),
            failure.params.get("reason").cloned().unwrap_or_default(),
        )
    };
    let key = "plugin.rulers-guides.errors.bad_args".to_owned();
    assert_eq!(
        code(ask(&mut host, 1, json!({"op": "paint"}))),
        (key.clone(), "op".to_owned())
    );
    assert_eq!(
        code(ask(&mut host, 1, json!("text"))),
        (key.clone(), "op".to_owned())
    );
    assert_eq!(
        code(ask(
            &mut host,
            1,
            json!({"op": "layout", "geometry": {"clientWidth": -1, "innerWidth": 1}})
        )),
        (key.clone(), "geometry".to_owned())
    );
    assert_eq!(
        code(ask(
            &mut host,
            1,
            json!({"op": "guide", "action": "add", "axis": "x", "geometry": geometry(375.0)})
        )),
        (key.clone(), "pos".to_owned())
    );
    assert_eq!(
        code(ask(
            &mut host,
            1,
            json!({"op": "guide", "action": "move", "id": 3, "pos": 1, "geometry": geometry(375.0)})
        )),
        (key, "guide.id".to_owned())
    );
    assert!(handle(&mut host, "{}").is_err());
}

#[test]
fn every_drawing_carries_the_colours_settings_and_overrides_give() {
    let mut host = host();
    host.settings
        .insert("colors.columnFill".to_owned(), "\"#0000ff33\"".to_owned());
    let answer = ask(
        &mut host,
        1,
        json!({"op": "layout", "geometry": geometry(375.0)}),
    )
    .expect("layout");
    assert_eq!(answer["layout"]["colors"]["columnFill"], "#0000ff33");
    assert_eq!(answer["layout"]["colors"]["guide"], "#00a2ff");
    // Without a project the page's copy of the overrides stands in, colours included.
    let mut bare = MemoryHost::new().grant(Capability::Views);
    bare.views = vec![view(1, "mobile", 375)];
    let answer = ask(
        &mut bare,
        1,
        json!({"op": "layout", "geometry": geometry(375.0),
               "overrides": {"colors": {"guide": "#ff00ff"}}}),
    )
    .expect("layout");
    assert_eq!(answer["layout"]["colors"]["guide"], "#ff00ff");
}
