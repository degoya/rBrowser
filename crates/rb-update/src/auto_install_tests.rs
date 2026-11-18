use super::*;

fn ready() -> Moment<'static> {
    Moment {
        mode: AutoMode::OnQuit,
        installs_itself: true,
        ready: Some("0.9.0-beta.3"),
        failed_before: false,
        other_windows: 0,
    }
}

#[test]
fn on_quit_installs_a_ready_download() {
    assert_eq!(on_quit(&ready()), Ok(()));
}

#[test]
fn quitting_installs_nothing_off_unsupported_unready_or_failed_before() {
    let off = Moment {
        mode: AutoMode::Off,
        ..ready()
    };
    assert_eq!(on_quit(&off), Err(Wait::Off));
    let scoop = Moment {
        installs_itself: false,
        ..ready()
    };
    assert_eq!(on_quit(&scoop), Err(Wait::Unsupported));
    // A download still running: quitting is never held up for it.
    let downloading = Moment {
        ready: None,
        ..ready()
    };
    assert_eq!(on_quit(&downloading), Err(Wait::NothingReady));
    let failed = Moment {
        failed_before: true,
        ..ready()
    };
    assert_eq!(on_quit(&failed), Err(Wait::FailedBefore));
    // Another project's window still runs: the download stays for the last one to quit.
    let others = Moment {
        other_windows: 2,
        ..ready()
    };
    assert_eq!(on_quit(&others), Err(Wait::OtherWindows));
    assert_eq!(Wait::NothingReady.as_str(), "nothing_ready");
}
