use std::sync::Arc;

use rb_config::UserSettings;
use rb_engine::ViewHost as _;
use tokio::runtime::Handle;

use super::*;
use crate::cdp::fake::FakeBrowser;
use crate::control::CoreSetup;
use crate::host::Host;
use crate::host::state::default_config;
use crate::host::ui::recording::{RecordingUi, UiCall};

fn core() -> (Arc<RecordingUi>, Arc<Core>) {
    let (_fake, router) = FakeBrowser::connect();
    let ui = Arc::new(RecordingUi::default());
    let host = Host::new(ui.clone(), router, Handle::current());
    host.open_config(default_config(), None).expect("opens");
    let setup = CoreSetup {
        user: UserSettings::default(),
        ..CoreSetup::default()
    };
    let core = Core::new(host, ui.clone(), Handle::current(), setup);
    (ui, core)
}

#[test]
fn windows_text_size_is_read_as_a_percentage_from_100_to_225() {
    assert_eq!(
        clamp_percent(None),
        100,
        "absent: Windows was never changed"
    );
    assert_eq!(clamp_percent(Some(100)), 100);
    assert_eq!(clamp_percent(Some(150)), 150);
    assert_eq!(clamp_percent(Some(225)), 225);
    assert_eq!(clamp_percent(Some(0)), 100, "below Windows' range");
    assert_eq!(clamp_percent(Some(99)), 100);
    assert_eq!(clamp_percent(Some(226)), 225, "above Windows' range");
    assert_eq!(clamp_percent(Some(u32::MAX)), 225);
}

#[test]
fn the_setting_switches_following_off() {
    let scale = TextScale::new().with_system(Some(200));
    assert_eq!(scale.percent(), 200);
    assert_eq!(scale.with_follow(false).percent(), 100);
    assert_eq!(scale.with_follow(false).with_follow(true).percent(), 200);
    assert_eq!(TextScale::new().percent(), 100, "before Windows was read");
}

#[test]
fn factor_1_0_1_5_and_2_25_give_the_shell_and_the_native_heights() {
    assert_eq!(
        heights(100),
        Heights {
            toolbar_row: 44,
            status_bar: 24,
            label: 20,
            footer_line: 18,
            footer_font_px: 11,
            dock_strip: (44, 36),
        }
    );
    assert_eq!(
        heights(150),
        Heights {
            toolbar_row: 66,
            status_bar: 36,
            label: 30,
            footer_line: 27,
            footer_font_px: 17,
            dock_strip: (66, 54),
        }
    );
    assert_eq!(
        heights(225),
        Heights {
            toolbar_row: 99,
            status_bar: 54,
            label: 45,
            footer_line: 41,
            footer_font_px: 25,
            dock_strip: (99, 81),
        }
    );
}

#[test]
fn a_size_rounds_to_the_nearest_dip_and_never_overflows() {
    assert_eq!(scale(18, 225), 41, "40.5 rounds up");
    assert_eq!(scale(11, 150), 17, "16.5 rounds up");
    assert_eq!(scale(10, 125), 13, "12.5 rounds up");
    assert_eq!(scale(0, 225), 0);
    assert_eq!(scale(u32::MAX, 225), u32::MAX, "saturates");
    assert_eq!(scale_i32(640, 150), 960);
    assert_eq!(scale_i32(-5, 150), 0, "a negative size is none");
}

#[test]
fn a_live_change_reports_only_a_new_percentage() {
    let mut scale = TextScale::new();
    assert_eq!(change(&mut scale, |s| s.with_system(Some(100))), None);
    assert_eq!(change(&mut scale, |s| s.with_system(Some(150))), Some(150));
    assert_eq!(
        change(&mut scale, |s| s.with_system(Some(150))),
        None,
        "unchanged"
    );
    assert_eq!(change(&mut scale, |s| s.with_follow(false)), Some(100));
    assert_eq!(
        change(&mut scale, |s| s.with_system(Some(225))),
        None,
        "Windows changed while the setting is off: the shell stays at 100 %"
    );
    assert_eq!(change(&mut scale, |s| s.with_follow(true)), Some(225));
    assert_eq!(change(&mut scale, |s| s.with_system(None)), Some(100));
}

#[tokio::test]
async fn a_page_of_the_shell_asks_for_the_percentage() {
    let (_ui, core) = core();
    let answer = ipc(&core, &ShellToCore::GetTextScale).expect("getTextScale is this module's");
    assert_eq!(answer, Ok(CoreToShell::TextScale { percent: 100 }));
    assert!(ipc(&core, &ShellToCore::GetState).is_none());
}

#[tokio::test]
async fn a_new_percentage_reaches_every_page_and_the_grid_is_laid_out_again() {
    let (ui, core) = core();
    let grids = ui
        .calls()
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count();
    let ids = |core: &Core| {
        core.host()
            .views()
            .iter()
            .map(|view| view.id)
            .collect::<Vec<_>>()
    };
    let before = ids(&core);
    announce(&core, 150);
    let calls = ui.calls();
    assert!(
        calls.iter().any(|call| matches!(
            call,
            UiCall::Shell(script) if script.contains("\"textScale\"") && script.contains("\"percent\":150")
        )),
        "the pages get the event: {calls:?}"
    );
    let after = calls
        .iter()
        .filter(|call| matches!(call, UiCall::Grid(_)))
        .count();
    assert_eq!(
        after,
        grids + 1,
        "the grid is sent again for the strips' heights"
    );
    assert_eq!(ids(&core), before, "every view is kept: nothing is rebuilt");
}
