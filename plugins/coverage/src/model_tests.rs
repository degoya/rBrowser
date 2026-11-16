use super::{FileRun, Function, Kind, Mode, Run, Status, ViewRun};

fn css(url: &str, total: u64, segments: &[(u32, u32)]) -> FileRun {
    FileRun {
        url: url.to_owned(),
        kind: Kind::Css,
        total,
        used: segments.iter().map(|(a, b)| u64::from(b - a)).sum(),
        segments: segments.to_vec(),
        rules: segments.to_vec(),
        ..FileRun::default()
    }
}

fn view(id: u32, files: Vec<FileRun>) -> ViewRun {
    ViewRun {
        view: id,
        label: format!("View {id}"),
        breakpoint: "desktop".to_owned(),
        width: 1280,
        engine: "chromium".to_owned(),
        url: "http://localhost/".to_owned(),
        files,
        ..ViewRun::default()
    }
}

#[test]
fn two_takes_of_a_session_add_up_their_used_parts() {
    let mut run = Run {
        mode: Mode::Interaction,
        started: 10,
        views: vec![view(1, vec![css("a.css", 100, &[(0, 10)])])],
        ..Run::default()
    };
    let take = Run {
        finished: 20,
        views: vec![view(
            1,
            vec![css("a.css", 100, &[(5, 30)]), css("b.css", 50, &[])],
        )],
        ..Run::default()
    };
    run.absorb(&take);
    let files = &run.views[0].files;
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].segments, vec![(0, 30)]);
    assert_eq!(files[0].used, 30);
    assert_eq!(files[0].rules, vec![(0, 10), (5, 30)]);
    assert_eq!(files[1].unused_percent(), 100.0);
    assert_eq!(run.finished, 20);
}

#[test]
fn a_function_that_ran_in_any_take_counts_as_executed() {
    let function = |executed| Function {
        name: "onClick".to_owned(),
        start: 10,
        end: 40,
        executed,
    };
    let mut first = FileRun {
        url: "app.js".to_owned(),
        kind: Kind::Js,
        total: 100,
        functions: vec![function(false)],
        ..FileRun::default()
    };
    let second = FileRun {
        functions: vec![function(true)],
        ..first.clone()
    };
    first.absorb(&second);
    assert!(first.functions[0].executed);
    assert_eq!(first.functions.len(), 1);
}

#[test]
fn unused_percent_is_rounded_to_a_tenth() {
    assert_eq!(css("a.css", 3, &[(0, 1)]).unused_percent(), 66.7);
    assert_eq!(css("a.css", 0, &[]).unused_percent(), 0.0);
}

#[test]
fn the_stored_shape_reads_back() {
    let run = Run {
        mode: Mode::Load,
        started: 1,
        finished: 2,
        running: false,
        views: vec![ViewRun {
            status: Status::Unavailable,
            engine: "firefox".to_owned(),
            ..view(2, Vec::new())
        }],
    };
    let text = serde_json::to_string(&run).expect("serialises");
    assert!(text.contains("\"mode\":\"load\""));
    assert!(text.contains("\"status\":\"unavailable\""));
    let back: Run = serde_json::from_str(&text).expect("reads back");
    assert_eq!(back, run);
    assert_eq!(back.measured().count(), 0);
}
