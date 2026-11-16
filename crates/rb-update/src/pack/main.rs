//! `rb-pack`: the publishing half of the self-update (RB-090-02) - a small binary, so signing a
//! release does not build the browser.
//!
//! * `rb-pack keygen --output <dir>` - a fresh Ed25519 pair: the private PKCS#8 PEM (`0600`,
//!   never overwritten, never printed) and the base64 public key to paste into
//!   `crates/rb-update/src/sign/roots.rs`.
//! * `rb-pack build ...` - the signed manifest of one release from its ZIP.
//! * `rb-pack verify <file> [--zip <file>] [--public-key <file>]` - against this build's root (or
//!   a given public key) and the ZIP's size and digest.
//!
//! The private key is read from `--key <file>` only; its path lives in the owner's documentation
//! (`docs/security/update-signing.md`), never in a script.
//!
//! Adapted from rCommander's `crates/rc-update/src/pack/` (`rc-pack`).

mod build;
mod keygen;
mod verify;

use std::process::ExitCode;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "rb-pack",
    version,
    about = "Sign and verify rBrowser's update manifests"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the Ed25519 key pair update manifests are signed with.
    Keygen(keygen::KeygenArgs),
    /// Build and sign the manifest of one release from its ZIP.
    Build(build::BuildArgs),
    /// Verify a signed manifest and, with `--zip`, the archive it lists.
    Verify(verify::VerifyArgs),
}

fn main() -> ExitCode {
    let outcome = match Cli::parse().command {
        Command::Keygen(args) => keygen::run(&args),
        Command::Build(args) => build::run(&args),
        Command::Verify(args) => verify::run(&args),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("rb-pack: {error:#}");
            ExitCode::FAILURE
        }
    }
}
