use super::{CSV_FILE, JSON_FILE, csv, field, write};
use crate::fake::FakeHost;
use crate::model::{FileRun, Kind, Mode, Run, Status, ViewRun};

fn run() -> Run {
    Run {
        mode: Mode::Load,
        started: 1,
        finished: 2,
        running: false,
        views: vec![
            ViewRun {
                view: 1,
                label: "Desktop".to_owned(),
                breakpoint: "desktop".to_owned(),
                width: 1280,
                engine: "chromium".to_owned(),
                url: "http://localhost/".to_owned(),
                status: Status::Measured,
                error: None,
                files: vec![FileRun {
                    url: "http://localhost/a,b.css".to_owned(),
                    kind: Kind::Css,
                    total: 200,
                    used: 50,
                    ..FileRun::default()
                }],
            },
            ViewRun {
                view: 2,
                label: "Firefox".to_owned(),
                engine: "firefox".to_owned(),
                status: Status::Unavailable,
                ..ViewRun::default()
            },
        ],
    }
}

#[test]
fn a_row_per_view_and_file() {
    let text = csv(&run());
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[1],
        "1,Desktop,desktop,1280,measured,http://localhost/,\"http://localhost/a,b.css\",css,false,200,50,150,75"
    );
    assert!(lines[2].starts_with("2,Firefox,,0,unavailable,"));
}

#[test]
fn fields_are_quoted_and_formulas_defused() {
    assert_eq!(field("plain"), "plain");
    assert_eq!(field("a\"b"), "\"a\"\"b\"");
    assert_eq!(field("=cmd()"), "'=cmd()");
}

#[test]
fn both_files_land_in_the_out_folder() {
    let mut host = FakeHost::new();
    let written = write(&mut host, &run()).expect("written");
    assert_eq!(written, [JSON_FILE, CSV_FILE]);
    assert!(host.written.contains_key("coverage.json"));
    assert!(host.written.contains_key("coverage.csv"));
}
