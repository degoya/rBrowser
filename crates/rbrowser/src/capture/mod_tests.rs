use super::*;

#[test]
fn every_command_has_a_preset_but_the_cycle() {
    for id in COMMANDS {
        assert_eq!(preset(id).is_some(), id != "capture.delay_cycle", "{id}");
    }
    assert!(preset("view.zoom_in").is_none());
}

#[test]
fn the_presets_are_the_job_files() {
    let overview = preset("capture.overview").expect("preset");
    assert_eq!(overview.target, Some(Target::All));
    assert_eq!(overview.overview, Some(true));
    let views = preset("capture.views").expect("preset");
    assert_eq!(views.target, None, "the active view");
    assert_eq!(views.mode, Some(CaptureMode::Viewport));
    let full = preset("capture.full_page").expect("preset");
    assert_eq!(
        (full.target, full.mode),
        (Some(Target::All), Some(CaptureMode::FullPage))
    );
    let element = preset("capture.element").expect("preset");
    assert_eq!(element.mode, Some(CaptureMode::Element));
    // A shortcut gives no arguments: the element is picked.
    let request = element
        .request(16, std::time::Duration::ZERO, true)
        .expect("a pick");
    assert_eq!(request.selector, None);
}

#[test]
fn errors_map_to_command_errors() {
    assert!(matches!(
        CommandError::from(CaptureError::Invalid("x".to_owned())),
        CommandError::InvalidArgs(_)
    ));
    assert!(matches!(
        CommandError::from(CaptureError::NoProject),
        CommandError::Failed(_)
    ));
}

#[test]
fn a_notice_carries_its_key_args_and_settings_link() {
    let CoreToShell::Notification(notification) = notice(
        NotificationLevel::Info,
        SAVED_KEY,
        &[("count", "2".to_owned())],
    ) else {
        panic!("a notification");
    };
    assert_eq!(notification.message_key, SAVED_KEY);
    assert_eq!(
        notification.args.get("count").map(String::as_str),
        Some("2")
    );
    assert_eq!(notification.settings_key.as_deref(), Some("capture"));
}

/// The grid may change during the delay: the views are read after it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_views_are_read_after_the_delay() {
    use std::time::Duration;

    use rb_engine::ViewHost as _;
    use tokio::runtime::Handle;

    use crate::cdp::fake::FakeBrowser;
    use crate::control::CoreSetup;
    use crate::host::state::default_config;
    use crate::host::ui::recording::RecordingUi;

    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let core = Core::new(host.clone(), ui, Handle::current(), CoreSetup::default());
    assert_eq!(host.views().len(), 3);
    let request = ScreenshotRequest {
        target: Some(Target::All),
        mode: CaptureMode::Viewport,
        selector: None,
        padding: 0,
        overview: false,
        delay: Duration::from_millis(300),
        save: false,
        views: None,
        folder: None,
        clean: rb_visual::CleanOverrides::default(),
        at: None,
        mask_passwords: false,
    };
    let capture = tokio::spawn({
        let core = core.clone();
        async move { screenshot(&core, &request).await }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    // Only the mobile cell is left; its view (3) stays, 1 and 2 close.
    let mut config = default_config();
    let mut layout = crate::host::state::layout_for(&config);
    layout.cells.retain(|cell| cell.breakpoint == "mobile");
    layout.columns = 1;
    layout.rows = 1;
    for cell in &mut layout.cells {
        cell.col = 1;
        cell.row = 1;
    }
    config.layout = layout;
    host.open_config(config, None).expect("reopens");
    assert_eq!(host.views().len(), 1);
    let error = capture
        .await
        .expect("the task ran")
        .expect_err("view 3 is not attached");
    let expected =
        EngineError::Invalid(format!("view {} is not attached yet", ViewId(3))).to_string();
    assert_eq!(error.to_string(), expected);
}

/// RB-090-85: a command's capture names its start unless the countdown of a delay or the pick
/// of an element already does.
#[test]
fn a_capture_names_its_start_once() {
    use std::time::Duration;

    let request = |mode: CaptureMode, delay_ms: u64| ScreenshotRequest {
        target: Some(Target::All),
        mode,
        selector: None,
        padding: 0,
        overview: false,
        delay: Duration::from_millis(delay_ms),
        save: true,
        views: None,
        folder: None,
        clean: rb_visual::CleanOverrides::default(),
        at: None,
        mask_passwords: false,
    };
    let Some(CoreToShell::Notification(started)) =
        capturing_notice(&request(CaptureMode::FullPage, 0))
    else {
        panic!("a notification");
    };
    assert_eq!(started.message_key, CAPTURING_KEY);
    assert_eq!(started.level, NotificationLevel::Info);
    assert!(capturing_notice(&request(CaptureMode::Viewport, 0)).is_some());
    assert!(capturing_notice(&request(CaptureMode::Viewport, 3000)).is_none());
    assert!(capturing_notice(&request(CaptureMode::Element, 0)).is_none());
}
