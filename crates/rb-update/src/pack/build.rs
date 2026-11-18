//! `rb-pack build`: the signed update manifest of one release from its ZIP
//! (`artifacts/windows/rbrowser-<version>-windows-x64.zip`, RB-090-01), signed with the key the
//! owner keeps offline and verified back under it before it is written.
//!
//! Adapted from rCommander's `crates/rc-update/src/pack/build.rs`: one archive kind, the notes a
//! plain text file (`scripts/release-assets.sh notes`), `min_version` instead of a schema change.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail, ensure};
use chrono::{Duration, SubsecRound as _, Utc};
use clap::Args;
use ed25519_dalek::{SigningKey, pkcs8::DecodePrivateKey as _};
use rb_update::manifest::{self, Artifact, MAX_NOTES_CHARS, MAX_VALIDITY_DAYS, kind};
use rb_update::sign::{self, EMBEDDED_KEYS, EmbeddedKey, TrustStore, UPDATE_KEY_ID};
use rb_update::{Channel, UpdateManifest};

#[derive(Args)]
pub(crate) struct BuildArgs {
    /// `stable` or `beta`; must match the version (a pre-release is beta).
    #[arg(long)]
    channel: String,
    /// The release's version, SemVer without `v`.
    #[arg(long)]
    version: String,
    /// The folder holding the release's archives (`rbrowser-<version>-windows-x64.zip`).
    #[arg(long)]
    assets: PathBuf,
    /// The https:// folder the archives are published under, ending in `/`.
    #[arg(long)]
    base_url: String,
    /// The PKCS#8 PEM private key `rb-pack keygen` wrote.
    #[arg(long)]
    key: PathBuf,
    /// A plain-text file with the release notes.
    #[arg(long)]
    notes: Option<PathBuf>,
    /// The oldest installed version that may switch to this one by itself.
    #[arg(long)]
    min_version: Option<String>,
    /// Days the manifest stays valid.
    #[arg(long, default_value_t = manifest::DEFAULT_VALIDITY_DAYS)]
    valid_days: i64,
    /// The sequence; the signing time in Unix seconds when not given.
    #[arg(long)]
    sequence: Option<u64>,
    /// Where the signed manifest is written.
    #[arg(long)]
    out: PathBuf,
}

/// Reads the PKCS#8 PEM private key at `path`; the key's text never reaches an error.
pub(crate) fn read_key_file(path: &Path) -> Result<SigningKey> {
    let pem = std::fs::read_to_string(path)
        .with_context(|| format!("read signing key {}", path.display()))?;
    SigningKey::from_pkcs8_pem(pem.trim())
        .map_err(|_| anyhow!("the signing key is no Ed25519 PKCS#8 private key"))
}

pub(crate) fn run(args: &BuildArgs) -> Result<()> {
    build_under(args, EMBEDDED_KEYS)
}

/// [`run`] against the update roots `roots` - the compiled-in ones but in its tests.
pub(crate) fn build_under(args: &BuildArgs, roots: &[EmbeddedKey]) -> Result<()> {
    ensure!(
        (1..=MAX_VALIDITY_DAYS).contains(&args.valid_days),
        "--valid-days must be between 1 and {MAX_VALIDITY_DAYS}"
    );
    let version = manifest::parse_version(&args.version)
        .with_context(|| format!("{} is not a SemVer version", args.version))?;
    let channel = Channel::parse(&args.channel)
        .with_context(|| format!("unknown channel {}", args.channel))?;
    let expected = if version.pre.is_empty() {
        Channel::Stable
    } else {
        Channel::Beta
    };
    ensure!(
        expected == channel,
        "{version} does not belong on the {channel} channel"
    );
    ensure!(
        manifest::is_plain_https(&args.base_url) && args.base_url.ends_with('/'),
        "--base-url must be an https:// folder ending in /"
    );
    let key = read_key_file(&args.key)?;
    check_against_root(&key, roots)?;
    let artifacts = artifacts(&args.assets, &version.to_string(), &args.base_url)?;
    let notes = match &args.notes {
        Some(path) => notes(
            &std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?,
        ),
        None => String::new(),
    };
    let issued_at = Utc::now().trunc_subsecs(0);
    let sequence = match args.sequence {
        Some(sequence) => sequence,
        None => u64::try_from(issued_at.timestamp()).context("the clock is before 1970")?,
    };
    let update = UpdateManifest {
        schema_version: manifest::UPDATE_MANIFEST_SCHEMA_VERSION,
        sequence,
        issued_at,
        not_after: issued_at + Duration::days(args.valid_days),
        channel,
        version: version.to_string(),
        released_at: issued_at,
        notes,
        artifacts,
        min_version: args.min_version.clone(),
    };
    write_signed(&key, &update, &args.out)
}

/// The notes as the manifest carries them: trimmed, cut at [`MAX_NOTES_CHARS`] characters.
pub(crate) fn notes(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_NOTES_CHARS {
        return text.to_owned();
    }
    eprintln!("warning: the notes are cut at {MAX_NOTES_CHARS} characters");
    let mut cut: String = text.chars().take(MAX_NOTES_CHARS - 4).collect();
    cut.push_str("\n...");
    cut
}

fn signing_trust(key: &SigningKey) -> Result<TrustStore> {
    let trust = TrustStore::new();
    trust.trust(UPDATE_KEY_ID.to_owned(), key.verifying_key())?;
    Ok(trust)
}

/// Signs, reads back under the signing key and writes `update` to `out`.
fn write_signed(key: &SigningKey, update: &UpdateManifest, out: &Path) -> Result<()> {
    let signed = manifest::sign(UPDATE_KEY_ID, key, update)?;
    manifest::verify_with(
        &signed,
        &signing_trust(key)?,
        update.channel,
        None,
        update.issued_at,
    )
    .map_err(|error| {
        anyhow!(
            "the signed manifest does not verify ({}): {error}",
            error.code()
        )
    })?;
    if let Some(parent) = out.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(out, &signed).with_context(|| format!("write {}", out.display()))?;
    println!(
        "signed {} {} with {} archives as sequence {} into {} (valid until {})",
        update.channel,
        update.version,
        update.artifacts.len(),
        update.sequence,
        out.display(),
        update.not_after.to_rfc3339(),
    );
    Ok(())
}

/// Refuses a key that is not the compiled-in root (the owner's, RB-090-14). A table without a
/// root only warns: the manifest then verifies nowhere until the public key is pasted in.
fn check_against_root(key: &SigningKey, table: &[EmbeddedKey]) -> Result<()> {
    let roots = sign::keys_at(table, Utc::now());
    match roots.iter().find(|root| root.key_id == UPDATE_KEY_ID) {
        Some(root) => ensure!(
            sign::decode_public_key(root.public_key)? == key.verifying_key(),
            "the key given is not the update root {UPDATE_KEY_ID} this build embeds"
        ),
        None => eprintln!(
            "warning: this build embeds no update root yet; paste the public key into \
             crates/rb-update/src/sign/roots.rs, or no installation verifies the manifest"
        ),
    }
    Ok(())
}

/// Every archive of `version` in `folder`, hashed and sized, sorted.
pub(crate) fn artifacts(folder: &Path, version: &str, base_url: &str) -> Result<Vec<Artifact>> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(folder).with_context(|| format!("list {}", folder.display()))? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((platform, arch)) = classify(&name, version) else {
            continue;
        };
        let bytes = std::fs::read(entry.path()).with_context(|| format!("read {name}"))?;
        found.push(Artifact {
            platform: platform.to_owned(),
            arch: arch.to_owned(),
            kind: kind::ZIP.to_owned(),
            url: format!("{base_url}{name}"),
            sha256: sign::hex_sha256(&bytes),
            size: u64::try_from(bytes.len())?,
        });
    }
    if found.is_empty() {
        bail!("{} holds no archive of {version}", folder.display());
    }
    found.sort_by(|left, right| (&left.platform, &left.arch).cmp(&(&right.platform, &right.arch)));
    Ok(found)
}

/// `(platform, arch)` of a package ZIP of `version`; `None` for anything else.
pub(crate) fn classify(name: &str, version: &str) -> Option<(&'static str, &'static str)> {
    let stem = name
        .strip_prefix(&format!("rbrowser-{version}-"))?
        .strip_suffix(".zip")?;
    match stem {
        "windows-x64" => Some(("windows", "x86_64")),
        _ => None,
    }
}

#[cfg(test)]
#[path = "build_tests.rs"]
mod tests;
