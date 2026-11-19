//! Where this window's update stands (RB-090-14): one state for the whole process, read by
//! `update_status`, the notices and the quit, and the answer `update_status` gives.

use std::path::PathBuf;
use std::sync::Mutex;

use rb_update::auto_install::{self, AutoMode, Moment};
use rb_update::{Channel, InstallKind, Offer, Progress, UpdateWay};
use serde_json::{Value, json};

/// What the update is doing.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Phase {
    /// Nothing checked yet in this run.
    #[default]
    Idle,
    Checking,
    /// The last check found nothing newer.
    UpToDate,
    /// A newer version is offered and nothing is downloaded.
    Available,
    Downloading,
    /// The offer is downloaded and verified.
    Ready,
    /// Handed to the updater; the window closes or the app ends.
    Installing,
    /// The last check, download or hand-over failed (`problem`).
    Failed,
}

impl Phase {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Checking => "checking",
            Self::UpToDate => "up_to_date",
            Self::Available => "available",
            Self::Downloading => "downloading",
            Self::Ready => "ready",
            Self::Installing => "installing",
            Self::Failed => "failed",
        }
    }

    /// Whether a check or a download runs now.
    pub(super) fn is_busy(self) -> bool {
        matches!(self, Self::Checking | Self::Downloading | Self::Installing)
    }
}

/// The process's update.
#[derive(Debug, Default)]
pub(super) struct State {
    pub phase: Phase,
    /// The newest offer of the last check that found one.
    pub offer: Option<Offer>,
    /// The verified download of [`Self::offer`].
    pub ready: Option<PathBuf>,
    /// The running download.
    pub progress: Option<Progress>,
    /// A person asked to install: the download ends in the hand-over and a restart.
    pub install_when_ready: bool,
    /// The code of the last failure.
    pub problem: Option<String>,
}

static STATE: Mutex<State> = Mutex::new(State {
    phase: Phase::Idle,
    offer: None,
    ready: None,
    progress: None,
    install_when_ready: false,
    problem: None,
});

/// Runs `change` on the process's state.
pub(super) fn with<T>(change: impl FnOnce(&mut State) -> T) -> T {
    change(&mut rb_util::lock(&STATE))
}

/// What the status answers besides the state: the facts of this installation.
#[derive(Clone, Debug)]
pub(super) struct Facts {
    pub enabled: bool,
    pub installed: String,
    pub channel: Channel,
    pub kind: InstallKind,
    pub auto_install: AutoMode,
    /// The last update to the offered version failed or was taken back.
    pub failed_before: bool,
    /// Other rBrowser windows (other projects) of this data folder that run now.
    pub other_windows: usize,
}

impl State {
    /// Whether quitting now installs the ready download, or why not.
    pub(super) fn on_quit(&self, facts: &Facts) -> Result<(), auto_install::Wait> {
        let ready = self
            .ready
            .as_ref()
            .and(self.offer.as_ref())
            .map(|offer| offer.version.as_str());
        auto_install::on_quit(&Moment {
            mode: facts.auto_install,
            installs_itself: facts.kind.installs_itself(),
            ready,
            failed_before: facts.failed_before,
            other_windows: facts.other_windows,
        })
    }

    /// The answer of `update_status`: no path, no address of this machine, no token.
    pub(super) fn status(&self, facts: &Facts) -> Value {
        let (way, command) = match facts.kind.update_way() {
            UpdateWay::SelfUpdate => ("self_update", None),
            UpdateWay::Command(command) => ("command", Some(command)),
            UpdateWay::DownloadOnly => ("download_only", None),
        };
        let offer = self.offer.as_ref();
        json!({
            "enabled": facts.enabled,
            "channel": facts.channel.as_str(),
            "installed": facts.installed,
            "offered": offer.map(|offer| offer.version.clone()),
            "releasedAt": offer.map(|offer| offer.released_at.to_rfc3339()),
            "size": offer.and_then(Offer::size),
            "notes": offer.map(|offer| offer.notes.clone()),
            "installKind": facts.kind.as_str(),
            "updateWay": way,
            "command": command,
            "state": self.phase.as_str(),
            "received": self.progress.as_ref().map(Progress::received),
            "problem": self.problem,
            "autoInstall": match facts.auto_install {
                AutoMode::Off => "off",
                AutoMode::OnQuit => "onQuit",
            },
            "installsOnQuit": self.on_quit(facts).is_ok(),
            "otherWindows": facts.other_windows,
        })
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
