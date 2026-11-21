use std::path::Path;

use rb_config::ProjectConfig;
use rb_engine::ViewId;

use super::*;

#[test]
fn a_report_captures_the_viewports_at_once_with_passwords_masked() {
    let screens = Path::new("/p/.rbrowser/out/reports/r/screens");
    let request = request(&[ViewId(1), ViewId(3)], screens, false);
    assert!(request.mask_passwords);
    assert_eq!(request.mode, CaptureMode::Viewport);
    assert_eq!(request.delay, Duration::ZERO);
    assert!(request.save && !request.overview);
    assert_eq!(request.views, Some(vec![ViewId(1), ViewId(3)]));
    assert_eq!(request.folder.as_deref(), Some(screens));
    assert!(super::request(&[ViewId(1)], screens, true).overview);
}

#[test]
fn an_image_is_named_relative_to_the_report() {
    let folder = Path::new("/p/.rbrowser/out/reports/r");
    let shot = ShotAnswer {
        view: Some(ViewId(2)),
        label: "mobile".to_owned(),
        width: 390,
        height: 844,
        path: Some(folder.join("screens/day/mobile.png").display().to_string()),
        png_base64: None,
        error: None,
    };
    let image = image_of(folder, &shot);
    assert_eq!(image.view, Some(2));
    assert_eq!(image.path.as_deref(), Some("screens/day/mobile.png"));
    let failed = ShotAnswer {
        path: None,
        error: Some("unsupported".to_owned()),
        ..shot
    };
    assert_eq!(
        image_of(folder, &failed).error.as_deref(),
        Some("unsupported")
    );
}

#[test]
fn the_environment_is_named_with_its_address() {
    let config: ProjectConfig = serde_json::from_value(serde_json::json!({
        "version": 1,
        "environments": [{ "id": "staging", "url": "https://staging.example.test" }]
    }))
    .expect("config");
    assert_eq!(
        environment_of(&config, Some("staging")),
        Some((
            "staging".to_owned(),
            "https://staging.example.test".to_owned()
        ))
    );
    assert_eq!(environment_of(&config, None), None);
    assert_eq!(
        environment_of(&config, Some("gone")),
        Some(("gone".to_owned(), String::new()))
    );
}

#[test]
fn the_preview_counts_the_images_taken_and_names_the_folder() {
    let image = |path: Option<&str>| Image {
        view: Some(1),
        path: path.map(str::to_owned),
        error: path.is_none().then(|| "unsupported".to_owned()),
    };
    let report = Report {
        images: vec![image(Some("screens/a.png")), image(None)],
        overview: Some(Image {
            view: None,
            ..image(Some("screens/overview.png"))
        }),
        ..Default::default()
    };
    let folder = Path::new("/p/.rbrowser/out/reports/r");
    let draft = draft_of(folder, "# Bug report\n", &report, true);
    assert_eq!(draft.images, 2, "the failed capture is no image");
    assert_eq!(draft.folder, folder.display().to_string());
    assert_eq!(draft.markdown, "# Bug report\n");
    assert!(draft.annotate);
    let none = draft_of(folder, "", &Report::default(), false);
    assert_eq!(none.images, 0);
    assert!(!none.annotate);
}
