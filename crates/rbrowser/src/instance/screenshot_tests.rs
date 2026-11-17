use std::sync::Mutex;

use rb_engine::Target;

use super::*;
use crate::capture::{CaptureMode, ShotAnswer};

#[derive(Default)]
struct Fake {
    seen: Mutex<Vec<(ScreenshotRequest, Duration)>>,
    fail: Option<String>,
}

impl Capturer for Fake {
    fn views(&self) -> Vec<(ViewId, Option<String>)> {
        vec![
            (ViewId(1), Some("mobile".to_owned())),
            (ViewId(2), Some("desktop".to_owned())),
            (ViewId(3), None),
        ]
    }

    fn padding(&self) -> u32 {
        16
    }

    fn capture(
        &self,
        request: ScreenshotRequest,
        budget: Duration,
    ) -> Result<ScreenshotAnswer, String> {
        self.seen.lock().expect("lock").push((request, budget));
        match &self.fail {
            Some(error) => Err(error.clone()),
            None => Ok(ScreenshotAnswer {
                shots: vec![ShotAnswer {
                    view: Some(ViewId(1)),
                    label: "Mobile".to_owned(),
                    width: 1179,
                    height: 2556,
                    path: Some("/p/mobile.png".to_owned()),
                    ..ShotAnswer::default()
                }],
                overview: None,
                folder: Some("/p".to_owned()),
            }),
        }
    }
}

fn shots_dir() -> String {
    std::env::temp_dir()
        .join("shots")
        .to_str()
        .expect("a Unicode temp folder")
        .to_owned()
}

fn args() -> ScreenshotArgs {
    ScreenshotArgs {
        target: Some(Target::All),
        mode: Some(CaptureMode::FullPage),
        delay_ms: Some(5000),
        save: Some(true),
        ..ScreenshotArgs::default()
    }
}

#[test]
fn the_named_views_are_captured_into_the_folder_given() {
    let fake = Fake::default();
    let out = shots_dir();
    let answer = run(
        &fake,
        &args(),
        &["3".to_owned(), "mobile".to_owned()],
        Some(out.as_str()),
    )
    .expect("captures");
    assert_eq!(answer["shots"][0]["path"], "/p/mobile.png");
    assert_eq!(answer["folder"], "/p");
    let seen = fake.seen.lock().expect("lock");
    let [(request, budget)] = seen.as_slice() else {
        panic!("one capture: {seen:?}");
    };
    assert_eq!(request.views, Some(vec![ViewId(1), ViewId(3)]));
    assert_eq!(request.folder, Some(std::path::PathBuf::from(&out)));
    assert_eq!(request.mode, CaptureMode::FullPage);
    assert_eq!(request.delay, Duration::from_secs(5));
    assert_eq!(request.padding, 16);
    assert!(request.save);
    assert_eq!(*budget, protocol::screenshot_budget(&args()));
}

#[test]
fn without_names_every_view_is_captured_into_the_project_folder() {
    let fake = Fake::default();
    run(&fake, &ScreenshotArgs::default(), &[], None).expect("captures");
    let seen = fake.seen.lock().expect("lock");
    assert_eq!(seen[0].0.views, None);
    assert_eq!(seen[0].0.folder, None);
    assert_eq!(seen[0].0.delay, Duration::ZERO);
    assert!(seen[0].0.save);
}

#[test]
fn what_the_grid_lacks_or_the_bounds_refuse_is_never_captured() {
    let fake = Fake::default();
    let unknown = run(&fake, &args(), &["tablet".to_owned()], None);
    assert_eq!(unknown, Err("no view shows tablet".to_owned()));
    let parent = format!("{}/../etc", shots_dir());
    for out in ["shots", parent.as_str()] {
        assert!(run(&fake, &args(), &[], Some(out)).is_err(), "{out}");
    }
    let base64 = ScreenshotArgs {
        save: Some(false),
        ..args()
    };
    assert!(run(&fake, &base64, &[], None).is_err());
    let picked = ScreenshotArgs {
        mode: Some(CaptureMode::Element),
        ..args()
    };
    assert!(run(&fake, &picked, &[], None).is_err(), "nobody clicks");
    assert!(fake.seen.lock().expect("lock").is_empty());
}

#[test]
fn a_failed_capture_names_its_reason() {
    let fake = Fake {
        fail: Some("there is no view to capture".to_owned()),
        ..Fake::default()
    };
    assert_eq!(
        run(&fake, &args(), &[], None),
        Err("there is no view to capture".to_owned())
    );
}
