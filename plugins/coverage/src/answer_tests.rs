use super::summary;
use crate::model::{FileRun, Kind, Mode, Run, ViewRun};

fn file(url: &str, total: u64, used: u64) -> FileRun {
    FileRun {
        url: url.to_owned(),
        kind: Kind::Css,
        total,
        used,
        ..FileRun::default()
    }
}

#[test]
fn files_are_listed_once_and_named_by_id_in_each_view() {
    let run = Run {
        mode: Mode::Load,
        views: vec![
            ViewRun {
                view: 1,
                files: vec![file("a.css", 100, 10), file("b.css", 10, 10)],
                ..ViewRun::default()
            },
            ViewRun {
                view: 2,
                files: vec![file("b.css", 10, 0)],
                ..ViewRun::default()
            },
        ],
        ..Run::default()
    };
    let answer = summary(&run, &[]);
    assert_eq!(answer["files"].as_array().map(Vec::len), Some(2));
    assert_eq!(answer["files"][1]["url"], "b.css");
    assert_eq!(answer["views"][1]["files"][0]["file"], 1);
    assert_eq!(answer["views"][1]["files"][0]["unusedPercent"], 100.0);
    assert_eq!(answer["views"][0]["files"][0]["unusedPercent"], 90.0);
    assert_eq!(answer["mode"], "load");
    assert_eq!(answer["truncated"], false);
    assert_eq!(answer["findingCount"], 0);
}
