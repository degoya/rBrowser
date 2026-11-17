use std::sync::Arc;

use rb_config::{ActivityBarShow, UserSettings};
use rb_ipc::{RecorderPhase, RecorderStatus};
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};
use crate::shell::SHELL_HEIGHT;

fn settings(position: ActivityBarPosition, mode: ActivityBarMode) -> ActivityBarSettings {
    ActivityBarSettings {
        position,
        mode,
        show: ActivityBarShow::default(),
    }
}

fn core(position: ActivityBarPosition, mode: ActivityBarMode) -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let mut user = UserSettings::default();
    user.ui.activity_bar = settings(position, mode);
    let setup = CoreSetup {
        user,
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

fn ask(core: &Core, place: Option<ActivityBarPosition>) -> Result<CoreToShell, Failure> {
    ipc(core, &ShellToCore::ActivityBar { place }).expect("activityBar is this module's")
}

fn recording(name: &str) -> RecorderStatus {
    RecorderStatus {
        phase: RecorderPhase::Recording,
        name: name.to_owned(),
        steps: 3,
    }
}

#[test]
fn a_request_gets_the_place_the_settings_name() {
    use ActivityBarPosition::{Bottom, Top};
    let top = settings(Top, ActivityBarMode::Bar);
    assert_eq!(checked_place(Some(Top), &top), Ok(Some(Top)));
    assert_eq!(checked_place(None, &top), Ok(None));
    assert!(checked_place(Some(Bottom), &top).is_err());
    let bottom = settings(Bottom, ActivityBarMode::Bar);
    assert_eq!(checked_place(Some(Bottom), &bottom), Ok(Some(Bottom)));
    // Indicator only: no room anywhere, giving it back always works.
    let indicator = settings(Top, ActivityBarMode::Indicator);
    assert!(checked_place(Some(Top), &indicator).is_err());
    assert_eq!(checked_place(None, &indicator), Ok(None));
}

// One test for the process-wide place: the steps follow each other.
#[tokio::test]
async fn the_bar_takes_its_height_from_the_views_and_gives_it_back() {
    let (ui, core) = core(ActivityBarPosition::Top, ActivityBarMode::Bar);
    assert_eq!(place(), None);
    assert_eq!(shell_view_height(), SHELL_HEIGHT);
    assert_eq!(status_view_height(), STATUS_BAR_HEIGHT);

    // Top: the toolbar's view grows, the state names the place after the room was made.
    assert_eq!(
        ask(&core, Some(ActivityBarPosition::Top)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(place(), Some(ActivityBarPosition::Top));
    assert_eq!(shell_view_height(), SHELL_HEIGHT + HEIGHT);
    assert_eq!(status_view_height(), STATUS_BAR_HEIGHT);
    assert_eq!(
        core.host().shell_state().activity_bar.place,
        Some(ActivityBarPosition::Top)
    );
    assert!(ui.calls().contains(&UiCall::StatusBar(true)));

    // Refused: the place stays.
    let refused = ask(&core, Some(ActivityBarPosition::Bottom)).expect_err("not the setting");
    assert_eq!(refused.0, IpcErrorCode::Refused);
    assert_eq!(place(), Some(ActivityBarPosition::Top));

    // The last activity ended: the views get the height back.
    assert_eq!(ask(&core, None), Ok(CoreToShell::Ack));
    assert_eq!(place(), None);
    assert_eq!(shell_view_height(), SHELL_HEIGHT);
    assert_eq!(core.host().shell_state().activity_bar.place, None);

    // Bottom with the status bar hidden: its view holds the bar alone, and goes with it.
    let (ui, core) = self::core(ActivityBarPosition::Bottom, ActivityBarMode::Bar);
    set_status_row(false);
    assert_eq!(status_view_height(), 0);
    assert!(!status_view_shown());
    assert_eq!(
        ask(&core, Some(ActivityBarPosition::Bottom)),
        Ok(CoreToShell::Ack)
    );
    assert_eq!(shell_view_height(), SHELL_HEIGHT);
    assert_eq!(status_view_height(), HEIGHT);
    assert!(ui.calls().contains(&UiCall::StatusBar(true)));
    set_status_row(true);
    assert_eq!(status_view_height(), STATUS_BAR_HEIGHT + HEIGHT);
    set_status_row(false);
    let before = ui.calls().len();
    assert_eq!(ask(&core, None), Ok(CoreToShell::Ack));
    assert_eq!(status_view_height(), 0);
    assert!(ui.calls()[before..].contains(&UiCall::StatusBar(false)));
    set_status_row(true);

    // Indicator only: the bar never gets room.
    let (_ui, core) = self::core(ActivityBarPosition::Top, ActivityBarMode::Indicator);
    let refused = ask(&core, Some(ActivityBarPosition::Top)).expect_err("indicator only");
    assert_eq!(refused.0, IpcErrorCode::Refused);
    assert_eq!(place(), None);
}

#[tokio::test]
async fn a_folded_entry_stands_in_the_state_until_it_is_opened() {
    let (_ui, core) = core(ActivityBarPosition::Top, ActivityBarMode::Bar);
    let fold = |activity, collapsed| {
        ipc(
            &core,
            &ShellToCore::CollapseActivity {
                activity,
                collapsed,
            },
        )
        .expect("collapseActivity is this module's")
    };
    assert_eq!(fold(Activity::Video, true), Ok(CoreToShell::Ack));
    assert_eq!(fold(Activity::Desync, true), Ok(CoreToShell::Ack));
    assert_eq!(
        core.host().shell_state().activity_bar.collapsed,
        [Activity::Video, Activity::Desync]
    );
    assert_eq!(fold(Activity::Video, false), Ok(CoreToShell::Ack));
    assert_eq!(
        core.host().shell_state().activity_bar.collapsed,
        [Activity::Desync]
    );
}

#[test]
fn a_stopped_recording_is_offered_for_play_until_closed_or_a_new_one_starts() {
    let mut shell = ShellState::default();
    // Starting: nothing to play yet.
    on_recorder(&mut shell, Some(&recording("login")));
    shell.recorder = Some(recording("login"));
    assert_eq!(shell.activity_bar.recorded, None);
    // Folding the entry while it records keeps the recording's entry folded.
    collapse(&mut shell, Activity::Recorder, true);
    assert_eq!(shell.activity_bar.collapsed, [Activity::Recorder]);
    collapse(&mut shell, Activity::Recorder, false);
    // Stopping offers it.
    on_recorder(&mut shell, None);
    shell.recorder = None;
    assert_eq!(shell.activity_bar.recorded.as_deref(), Some("login"));
    // A new recording ends the offer.
    on_recorder(&mut shell, Some(&recording("cart")));
    shell.recorder = Some(recording("cart"));
    assert_eq!(shell.activity_bar.recorded, None);
    on_recorder(&mut shell, None);
    shell.recorder = None;
    assert_eq!(shell.activity_bar.recorded.as_deref(), Some("cart"));
    // Closing the entry (or playing it) ends the offer and folds nothing.
    collapse(&mut shell, Activity::Recorder, true);
    assert_eq!(shell.activity_bar.recorded, None);
    assert!(shell.activity_bar.collapsed.is_empty());
}

#[tokio::test]
async fn the_running_video_reaches_the_state_and_leaves_it() {
    let (_ui, core) = core(ActivityBarPosition::Top, ActivityBarMode::Bar);
    publish_video(
        core.host(),
        Some((vec![ViewId(1), ViewId(2)], 1_763_380_800_000, 600)),
    );
    let video = core.host().shell_state().video.expect("a video runs");
    assert_eq!(video.views, [ViewId(1), ViewId(2)]);
    assert_eq!(video.started_at_ms, 1_763_380_800_000);
    assert_eq!(video.max_duration_sec, 600);
    let snapshot = crate::ipc::snapshot(core.host());
    assert_eq!(snapshot.video.map(|video| video.views.len()), Some(2));
    publish_video(core.host(), None);
    assert_eq!(core.host().shell_state().video, None);
}

#[tokio::test]
async fn other_messages_are_not_this_modules() {
    let (_ui, core) = core(ActivityBarPosition::Top, ActivityBarMode::Bar);
    assert!(ipc(&core, &ShellToCore::GetState).is_none());
}
