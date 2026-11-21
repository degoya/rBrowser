use rb_update::{Artifact, OffsetDateTime};

use super::*;

fn offer(version: &str) -> Offer {
    Offer {
        version: version.to_owned(),
        channel: Channel::Beta,
        released_at: OffsetDateTime::from_unix_timestamp(1_790_000_000).expect("time"),
        notes: "- Self-update".to_owned(),
        artifact: Some(Artifact {
            platform: "windows".to_owned(),
            arch: "x86_64".to_owned(),
            kind: "zip".to_owned(),
            url: "https://github.com/degoya/rBrowser/releases/download/v0.9.0-beta.3/rbrowser-0.9.0-beta.3-windows-x64.zip"
                .to_owned(),
            sha256: "ab".repeat(32),
            size: 210_000_000,
        }),
    }
}

fn facts(kind: InstallKind, auto_install: AutoMode) -> Facts {
    Facts {
        enabled: true,
        installed: "0.9.0-beta.2".to_owned(),
        channel: Channel::Beta,
        kind,
        auto_install,
        failed_before: false,
        other_windows: 0,
    }
}

#[test]
fn the_status_names_the_offer_and_how_this_installation_takes_it() {
    let state = State {
        phase: Phase::Available,
        offer: Some(offer("0.9.0-beta.3")),
        ..State::default()
    };
    let status = state.status(&facts(InstallKind::Portable, AutoMode::Off));
    assert_eq!(status["installed"], "0.9.0-beta.2");
    assert_eq!(status["offered"], "0.9.0-beta.3");
    assert_eq!(status["channel"], "beta");
    assert_eq!(status["size"], 210_000_000);
    assert_eq!(status["notes"], "- Self-update");
    assert_eq!(status["installKind"], "portable");
    assert_eq!(status["updateWay"], "self_update");
    assert_eq!(status["command"], Value::Null);
    assert_eq!(status["state"], "available");
    assert_eq!(status["installsOnQuit"], false);
    let scoop = state.status(&facts(InstallKind::Scoop, AutoMode::Off));
    assert_eq!(scoop["updateWay"], "command");
    assert_eq!(scoop["command"], "scoop update rbrowser");
    let unknown = state.status(&facts(InstallKind::Unknown, AutoMode::Off));
    assert_eq!(unknown["updateWay"], "download_only");
}

/// The answer holds no path of this machine: the download's folder stays out.
#[test]
fn the_status_carries_no_local_path() {
    let state = State {
        phase: Phase::Ready,
        offer: Some(offer("0.9.0-beta.3")),
        ready: Some(PathBuf::from(
            "/home/someone/.local/share/rBrowser/update/download/x.zip",
        )),
        ..State::default()
    };
    let text = state
        .status(&facts(InstallKind::Portable, AutoMode::OnQuit))
        .to_string();
    assert!(!text.contains("someone"), "{text}");
}

/// The quit installs a ready download with `onQuit` only, and never one still downloading.
#[test]
fn quitting_installs_only_a_ready_download_with_on_quit() {
    let downloading = State {
        phase: Phase::Downloading,
        offer: Some(offer("0.9.0-beta.3")),
        ..State::default()
    };
    let on_quit = facts(InstallKind::Portable, AutoMode::OnQuit);
    assert_eq!(
        downloading.on_quit(&on_quit),
        Err(auto_install::Wait::NothingReady)
    );
    let ready = State {
        phase: Phase::Ready,
        offer: Some(offer("0.9.0-beta.3")),
        ready: Some(PathBuf::from("x.zip")),
        ..State::default()
    };
    assert_eq!(ready.on_quit(&on_quit), Ok(()));
    assert_eq!(ready.status(&on_quit)["installsOnQuit"], true);
    assert_eq!(
        ready.on_quit(&facts(InstallKind::Portable, AutoMode::Off)),
        Err(auto_install::Wait::Off)
    );
    assert_eq!(
        ready.on_quit(&facts(InstallKind::Winget, AutoMode::OnQuit)),
        Err(auto_install::Wait::Unsupported)
    );
    // Another project's window runs: the download stays for the last window to quit.
    let others = Facts {
        other_windows: 1,
        ..on_quit.clone()
    };
    assert_eq!(
        ready.on_quit(&others),
        Err(auto_install::Wait::OtherWindows)
    );
    assert_eq!(ready.status(&others)["otherWindows"], 1);
}

#[test]
fn a_check_a_download_and_a_hand_over_are_busy() {
    assert!(Phase::Checking.is_busy());
    assert!(Phase::Downloading.is_busy());
    assert!(Phase::Installing.is_busy());
    assert!(!Phase::Available.is_busy());
    assert!(!Phase::Ready.is_busy());
}
