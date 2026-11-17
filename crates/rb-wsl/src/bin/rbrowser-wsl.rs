//! `rbrowser-wsl`: rBrowser's helper inside a WSL distribution, started by the Windows app as
//! `wsl.exe -d <distro> -e <path> <subcommand>` and spoken to over stdio (`rb_wsl::protocol`).
//! Built static for `x86_64-unknown-linux-musl`, so it runs in any distribution without further
//! libraries. `watch` reports the changes of a project folder (RB-030-05, inotify);
//! `cdp-proxy` (RB-030-06) listens on `127.0.0.1` in the distribution and tunnels every
//! connection over stdio to rBrowser's CDP proxy.

use std::io::{self, BufReader, Write as _};
use std::path::Path;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use rb_wsl::cdp_proxy::helper::serve_cdp_proxy;
use rb_wsl::serve::Served;
use rb_wsl::watch::run::serve_watch;

/// Exit status of a refused handshake or a bad argument.
const EXIT_REFUSED: u8 = 2;

#[derive(Parser)]
#[command(
    name = "rbrowser-wsl",
    version,
    about = "rBrowser's helper inside WSL (stdio protocol)"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Watch a project folder and report changes (RB-030-05).
    Watch {
        /// The absolute Linux path of the folder.
        path: String,
    },
    /// Tunnel the CDP proxy into the distribution (RB-030-06).
    CdpProxy {
        /// The port on 127.0.0.1 inside the distribution; a free one when it is taken or 0.
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Command::Watch { path } = &cli.command
        && !path.starts_with('/')
    {
        let _ = writeln!(
            io::stderr(),
            "rbrowser-wsl: watch needs an absolute Linux path"
        );
        return ExitCode::from(EXIT_REFUSED);
    }
    let version = env!("CARGO_PKG_VERSION");
    let served = match &cli.command {
        Command::Watch { path } => serve_watch(
            BufReader::new(io::stdin()),
            io::stdout().lock(),
            version,
            Path::new(path),
        ),
        Command::CdpProxy { port } => tunnel(version, *port),
    };
    match served {
        Ok(Served::Shutdown | Served::Closed) => ExitCode::SUCCESS,
        Ok(Served::HandshakeFailed(error)) => {
            let _ = writeln!(io::stderr(), "rbrowser-wsl: {error}");
            ExitCode::from(EXIT_REFUSED)
        }
        Err(error) => {
            let _ = writeln!(io::stderr(), "rbrowser-wsl: {error}");
            ExitCode::FAILURE
        }
    }
}

/// `cdp-proxy`: the tunnel on a single-threaded runtime.
fn tunnel(version: &str, port: u16) -> io::Result<Served> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let served = runtime.block_on(serve_cdp_proxy(
        tokio::io::BufReader::new(tokio::io::stdin()),
        tokio::io::stdout(),
        version,
        port,
    ));
    // The read of stdin may still block in a thread of its own: do not wait for it.
    runtime.shutdown_background();
    served
}
