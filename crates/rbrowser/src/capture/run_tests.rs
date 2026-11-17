use std::sync::Mutex;

use super::*;
use crate::cdp::lock;

/// Answers a solid PNG of the view's size times the DPR and `scale`; fails for `broken`.
struct FakeShooter {
    views: Vec<ViewMeta>,
    broken: Option<ViewId>,
    scales: Mutex<Vec<f64>>,
}

#[async_trait]
impl Shooter for FakeShooter {
    async fn shoot(&self, view: ViewId, _shot: &Shot, scale: f64) -> Result<Vec<u8>, EngineError> {
        lock(&self.scales).push(scale);
        if Some(view) == self.broken {
            return Err(EngineError::Invalid("the view crashed".to_owned()));
        }
        let meta = self
            .views
            .iter()
            .find(|meta| meta.id == view)
            .expect("known view");
        let dpr = if meta.dpr > 0.0 { meta.dpr } else { 1.0 };
        let side = |css: u32| (f64::from(css) * dpr * scale).round() as u32;
        let image = rb_visual::Rgba::new(side(meta.width), side(meta.height), [200, 0, 0, 255])
            .expect("small");
        Ok(encode_png(&image).expect("encoded"))
    }
}

fn view(id: u32, name: &str, width: u32, height: u32, dpr: f64) -> ViewMeta {
    ViewMeta {
        id: ViewId(id),
        label: name.to_uppercase(),
        name: name.to_owned(),
        width,
        height,
        dpr,
        engine: "chromium".to_owned(),
    }
}

fn views() -> Vec<ViewMeta> {
    vec![
        view(1, "mobile", 20, 40, 2.0),
        view(2, "desktop", 60, 30, 1.0),
    ]
}

fn shooter(broken: Option<u32>) -> FakeShooter {
    FakeShooter {
        views: views(),
        broken: broken.map(ViewId),
        scales: Mutex::new(Vec::new()),
    }
}

fn context(folder: Option<PathBuf>) -> RunContext {
    RunContext {
        env: Some("staging".to_owned()),
        env_color: Some([22, 163, 74]),
        pattern: "{date}/{env}/{breakpoint}".to_owned(),
        overview_labels: true,
        folder,
        stamp: Stamp::from_unix(1_794_312_187),
    }
}

#[tokio::test]
async fn every_view_gets_a_file_named_by_the_pattern() {
    let dir = tempfile::tempdir().expect("temp dir");
    let context = context(Some(dir.path().to_path_buf()));
    let answer = run(
        &shooter(None),
        &views(),
        &Shot::Viewport,
        CaptureMode::Viewport,
        false,
        &context,
    )
    .await
    .expect("captured");
    let run_folder = dir.path().join("2026-11-10_12-03-07").join("staging");
    assert_eq!(
        answer.paths(),
        vec![
            run_folder.join("mobile.png").display().to_string(),
            run_folder.join("desktop.png").display().to_string(),
        ]
    );
    assert_eq!(answer.folder, Some(run_folder.display().to_string()));
    assert_eq!(
        (answer.shots[0].width, answer.shots[0].height),
        (40, 80),
        "at the DPR"
    );
    assert!(run_folder.join("mobile.png").is_file());
}

#[tokio::test]
async fn two_views_of_one_breakpoint_do_not_overwrite_each_other() {
    let dir = tempfile::tempdir().expect("temp dir");
    let twins = vec![
        view(1, "mobile", 20, 40, 2.0),
        view(2, "mobile", 20, 40, 2.0),
    ];
    let shooter = FakeShooter {
        views: twins.clone(),
        broken: None,
        scales: Mutex::new(Vec::new()),
    };
    let answer = run(
        &shooter,
        &twins,
        &Shot::Viewport,
        CaptureMode::Viewport,
        false,
        &context(Some(dir.path().to_path_buf())),
    )
    .await
    .expect("captured");
    let paths = answer.paths();
    assert!(paths[0].ends_with("mobile.png"));
    assert!(paths[1].ends_with("mobile-2.png"));
}

#[tokio::test]
async fn a_failing_view_is_named_and_the_others_go_on() {
    let dir = tempfile::tempdir().expect("temp dir");
    let context = context(Some(dir.path().to_path_buf()));
    let answer = run(
        &shooter(Some(1)),
        &views(),
        &Shot::FullPage,
        CaptureMode::FullPage,
        false,
        &context,
    )
    .await
    .expect("one view captured");
    assert!(
        answer.shots[0]
            .error
            .as_deref()
            .is_some_and(|error| error.contains("crashed"))
    );
    assert!(answer.shots[0].path.is_none());
    assert!(answer.shots[1].path.is_some());
    let all_broken = FakeShooter {
        views: vec![view(1, "mobile", 20, 40, 2.0)],
        broken: Some(ViewId(1)),
        scales: Mutex::new(Vec::new()),
    };
    let only = vec![view(1, "mobile", 20, 40, 2.0)];
    let error = run(
        &all_broken,
        &only,
        &Shot::Viewport,
        CaptureMode::Viewport,
        false,
        &context,
    )
    .await
    .expect_err("nothing captured");
    assert!(matches!(error, CaptureError::Failed(_)));
}

#[tokio::test]
async fn without_a_folder_the_images_come_back_as_base64() {
    let answer = run(
        &shooter(None),
        &views(),
        &Shot::Viewport,
        CaptureMode::Viewport,
        false,
        &context(None),
    )
    .await
    .expect("captured");
    assert!(answer.paths().is_empty());
    assert_eq!(answer.folder, None);
    let data = answer.shots[1].png_base64.as_deref().expect("inline");
    let png = base64::engine::general_purpose::STANDARD
        .decode(data)
        .expect("base64");
    assert_eq!(png_size(&png), Some((60, 30)));
}

#[tokio::test]
async fn the_overview_is_one_image_of_all_views_at_css_size() {
    let dir = tempfile::tempdir().expect("temp dir");
    let shooter = shooter(None);
    let context = context(Some(dir.path().to_path_buf()));
    let answer = run(
        &shooter,
        &views(),
        &Shot::Viewport,
        CaptureMode::Viewport,
        true,
        &context,
    )
    .await
    .expect("captured");
    assert_eq!(*lock(&shooter.scales), vec![0.5, 1.0], "1 / dpr each");
    assert!(answer.shots.iter().all(|shot| shot.path.is_none()));
    assert_eq!((answer.shots[0].width, answer.shots[0].height), (20, 40));
    let overview = answer.overview.clone().expect("an overview");
    let placed = rb_visual::overview::layout(&[(20, 40), (60, 30)], true);
    assert_eq!(
        (overview.width, overview.height),
        (placed.width, placed.height)
    );
    let path = overview.path.expect("a file");
    assert!(path.ends_with("overview.png"), "{path}");
    assert_eq!(answer.paths(), vec![path.as_str()]);
}

#[tokio::test]
async fn no_views_is_an_error() {
    let error = run(
        &shooter(None),
        &[],
        &Shot::Viewport,
        CaptureMode::Viewport,
        false,
        &context(None),
    )
    .await
    .expect_err("nothing to capture");
    assert!(matches!(error, CaptureError::NoViews));
}
