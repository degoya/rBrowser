//! A stand-in for rBrowser in `scripts/self-update-smoke.sh` (RB-090-14): what the app does
//! around an update, without CEF, so the real `rbrowser-updater` can switch, start, prove and take
//! back real binaries on Windows and Linux runners that have no display.
//!
//! * `update_smoke_program --data <dir>`: a start of the program - ends or takes back an
//!   interrupted update (`recover_at_start`; a taken-back newer program starts the older one and
//!   ends), takes its share of the instance lock, proves a switched update (`confirm_started`,
//!   refused in a debug build with `RB_UPDATE_TEST_FAIL_START`), appends
//!   `<version> proven=<bool>` to `<data>/smoke-starts.log`, and ends two seconds later.
//! * `update_smoke_program --data <dir> hand-over --zip <file> --version <v>`: the app's
//!   "Install and restart" (or, with `--on-quit`, the hand-over at quit): the plan for the ZIP,
//!   the journal, the shipped updater started; then it ends.
//!
//! Built twice by the smoke, the second time after `scripts/set-version.sh` raised the version.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use rb_update::install::InstanceLock;
use rb_update::install::handover::{self, Running};
use rb_update::install::process::spawn_detached;
use rb_update::install::recover::{self, Recovery};
use rb_update::{Artifact, InstallKind};

const VERSION: &str = env!("CARGO_PKG_VERSION");
/// What a start records, one line per start.
const STARTS_LOG: &str = "smoke-starts.log";

#[derive(Parser)]
#[command(name = "update_smoke_program", version)]
struct Cli {
    /// The local data folder (the journal lives in its `update/`).
    #[arg(long)]
    data: PathBuf,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Hand the ZIP over to the updater, as the app's install does.
    HandOver {
        /// The new version's ZIP, absolute.
        #[arg(long)]
        zip: PathBuf,
        /// Its version.
        #[arg(long)]
        version: String,
        /// How long the updater waits for the new version's proof.
        #[arg(long, default_value_t = 30)]
        confirm_timeout: u64,
        /// The hand-over at quit: the updater switches and starts nothing.
        #[arg(long)]
        on_quit: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let outcome = std::env::current_exe()
        .context("locate this program")
        .and_then(|exe| match cli.command {
            None => start(&cli.data, &exe),
            Some(Command::HandOver {
                zip,
                version,
                confirm_timeout,
                on_quit,
            }) => hand_over(&cli.data, &exe, &zip, &version, confirm_timeout, on_quit),
        });
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("update_smoke_program {VERSION}: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn start(data: &Path, exe: &Path) -> Result<()> {
    if let Recovery::Restart(program) = recover::recover_at_start(data, exe, VERSION)? {
        let folder = program.parent().context("the program has no folder")?;
        let data_arg = data.to_string_lossy().into_owned();
        spawn_detached(&program, &["--data", data_arg.as_str()], folder, None)?;
        record(data, "restarted the previous version")?;
        return Ok(());
    }
    let _share = InstanceLock::share(data)?;
    let proven = recover::confirm_started(data, exe, VERSION)?;
    record(data, &format!("proven={proven}"))?;
    std::thread::sleep(Duration::from_secs(2));
    Ok(())
}

fn record(data: &Path, what: &str) -> Result<()> {
    std::fs::create_dir_all(data).with_context(|| format!("create {}", data.display()))?;
    let path = data.join(STARTS_LOG);
    let mut log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open {}", path.display()))?;
    writeln!(log, "{VERSION} {what}").with_context(|| format!("write {}", path.display()))
}

fn hand_over(
    data: &Path,
    exe: &Path,
    zip: &Path,
    version: &str,
    confirm_timeout: u64,
    on_quit: bool,
) -> Result<()> {
    let bytes = std::fs::read(zip).with_context(|| format!("read {}", zip.display()))?;
    let artifact = Artifact {
        platform: "smoke".to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        kind: "zip".to_owned(),
        url: "https://github.com/degoya/rBrowser/releases/download/smoke/smoke.zip".to_owned(),
        sha256: rb_update::sign::hex_sha256(&bytes),
        size: u64::try_from(bytes.len()).context("the ZIP's size")?,
    };
    let data_arg = data.to_string_lossy().into_owned();
    let running = Running {
        executable: exe.to_path_buf(),
        version: VERSION.to_owned(),
        kind: InstallKind::Portable,
        data_dir: data.to_path_buf(),
        args: vec!["--data".to_owned(), data_arg],
    };
    let mut plan = handover::plan(&running, version, &artifact, zip, !on_quit)?;
    plan.confirm_timeout_secs = confirm_timeout;
    handover::hand_over(plan)?;
    record(data, &format!("handed over {version}"))
}
