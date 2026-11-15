//! `rb-pack keygen`: a fresh Ed25519 key pair for update manifests. The private half is written
//! once (`create_new`, `0600` on Unix) and never printed; the public half is printed and written
//! beside it, for `crates/rb-update/src/sign/roots.rs`.
//!
//! Adapted from rCommander's `crates/rc-update/src/pack/keygen.rs`: one role.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::Args;
use ed25519_dalek::{
    SigningKey,
    pkcs8::{EncodePrivateKey, spki::der::pem::LineEnding},
};
use rand::Rng as _;

/// The file stem of the pair: `rbrowser-update.key` and `rbrowser-update.pub`.
const STEM: &str = "rbrowser-update";

#[derive(Args)]
pub(crate) struct KeygenArgs {
    /// The folder the key files are written to, outside the repository; an existing key file is
    /// never overwritten.
    #[arg(long)]
    output: PathBuf,
}

/// The two files in `folder`: the private key and the public one.
pub(crate) fn key_files(folder: &Path) -> (PathBuf, PathBuf) {
    (
        folder.join(format!("{STEM}.key")),
        folder.join(format!("{STEM}.pub")),
    )
}

pub(crate) fn run(args: &KeygenArgs) -> Result<()> {
    let (private, public) = key_files(&args.output);
    let encoded = generate(&args.output)?;
    println!("key id:     {}", rb_update::sign::UPDATE_KEY_ID);
    println!("public key: {encoded}");
    println!(
        "private:    {} (keep it offline; never commit it)",
        private.display()
    );
    println!("public:     {}", public.display());
    Ok(())
}

/// Writes a fresh pair into `folder` and returns the base64 public key.
pub(crate) fn generate(folder: &Path) -> Result<String> {
    let mut seed = [0_u8; 32];
    rand::rng().fill_bytes(&mut seed);
    let key = SigningKey::from_bytes(&seed);
    let pem = key
        .to_pkcs8_pem(LineEnding::LF)
        .map_err(|error| anyhow::anyhow!("encode the key as PKCS#8: {error}"))?;
    let encoded = STANDARD.encode(key.verifying_key().as_bytes());
    std::fs::create_dir_all(folder).with_context(|| format!("create {}", folder.display()))?;
    let (private, public) = key_files(folder);
    write_new(&private, pem.as_bytes(), true)?;
    write_new(&public, format!("{encoded}\n").as_bytes(), false)?;
    Ok(encoded)
}

/// Creates `path`, refusing an existing one; `private` makes it `0600` on Unix.
fn write_new(path: &Path, bytes: &[u8], private: bool) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(if private { 0o600 } else { 0o644 });
    }
    #[cfg(not(unix))]
    let _ = private;
    let mut file = options
        .open(path)
        .with_context(|| format!("create {} (it must not exist)", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
#[path = "keygen_tests.rs"]
mod tests;
