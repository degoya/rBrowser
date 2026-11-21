//! `rb-plugin-pack`: makes signing keys, packs and signs `.rbplugin` packages, verifies them and
//! runs the conformance checks - what `scripts/build-plugins.sh` and a plugin author run
//! (`docs/plugins.md`, "Signing keys", "SDK").
//!
//! ```text
//! rb-plugin-pack keygen --out <folder outside the repository>
//! rb-plugin-pack package <plugin folder> --component <component.wasm> --out <file.rbplugin>
//!                        [--key <private.pem>]           (unsigned without: development only)
//! rb-plugin-pack verify <file.rbplugin> [--trust <key_id>=<base64>]... [--dev]
//! rb-plugin-pack conformance <file.rbplugin> [--trust <key_id>=<base64>]... [--dev]
//! rb-plugin-pack index <folder of .rbplugin> --out <rbrowser-plugin-index.json> --key <private.pem>
//!                      --key-id <id> --sequence <n> [--base-url <https://...>]
//!                      [--validity-days <n>] [--trust <key_id>=<base64>]...
//!                      [--categories <first-party.json>] [--previous <old index.json>]
//! ```
//!
//! `index` (RB-091-01) lists every package of the folder that verifies under the compiled-in roots
//! and the `--trust` keys, and signs the index with the repository key: the official catalogue's
//! with the plugin release key, a third-party repository's with its own. RB-091-04: `--categories`
//! names each plugin's group from `plugins/first-party.json`, and `--previous` carries the
//! previous index's newest entry of every plugin per contract this one does not build for (its
//! signature must verify under the same key; its expiry does not matter).
//!
//! The private key is read from a file and never printed; `keygen` writes it with owner-only
//! permissions where the platform has them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use rb_plugin_host::{
    DEFAULT_VALIDITY_DAYS, IndexSigners, MANIFEST_MEMBER, PACKAGE_EXTENSION,
    PLUGIN_INDEX_SCHEMA_VERSION, PluginIndex, PluginVerifier, SETTINGS_SCHEMA_MEMBER, carry_over,
    describe_package, generate_signing_key, load_signing_key_pem, package_plugin, run_conformance,
    sign_index, verify_index_signature, verify_package,
};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("keygen") => keygen(&args[1..]),
        Some("package") => package(&args[1..]),
        Some("verify") => verify(&args[1..]),
        Some("conformance") => conformance(&args[1..]),
        Some("index") => index(&args[1..]),
        _ => {
            eprintln!(
                "usage: rb-plugin-pack keygen --out <dir> | package <dir> --component <wasm> \
                 --out <file> [--key <pem>] | verify <file> [--trust id=key] [--dev] | \
                 conformance <file> [--trust id=key] [--dev] | index <dir> --out <file> \
                 --key <pem> --key-id <id> --sequence <n> [--base-url <url>] \
                 [--validity-days <n>] [--trust id=key] [--categories <json>] \
                 [--previous <index>]"
            );
            std::process::exit(2);
        }
    }
}

/// The value after `--name`, if given.
fn option<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .map(String::as_str)
}

fn keygen(args: &[String]) -> Result<()> {
    let out = PathBuf::from(option(args, "--out").context("keygen needs --out <dir>")?);
    std::fs::create_dir_all(&out).with_context(|| format!("create {}", out.display()))?;
    let key = generate_signing_key()?;
    let private = out.join("plugin-signing.pem");
    if private.exists() {
        bail!(
            "{} exists; a signing key is never overwritten",
            private.display()
        );
    }
    std::fs::write(&private, key.private_pem.as_bytes())
        .with_context(|| format!("write {}", private.display()))?;
    restrict(&private)?;
    println!(
        "private key: {} (keep it out of every repository)",
        private.display()
    );
    println!("public key:  {}", key.public_base64);
    Ok(())
}

#[cfg(unix)]
fn restrict(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .with_context(|| format!("restrict {}", path.display()))
}

#[cfg(not(unix))]
fn restrict(_path: &Path) -> Result<()> {
    Ok(())
}

fn package(args: &[String]) -> Result<()> {
    let folder = PathBuf::from(args.first().context("package needs the plugin folder")?);
    let component_path = option(args, "--component").context("package needs --component")?;
    let out = PathBuf::from(option(args, "--out").context("package needs --out")?);
    let manifest = std::fs::read(folder.join(MANIFEST_MEMBER))
        .with_context(|| format!("read {}", folder.join(MANIFEST_MEMBER).display()))?;
    let component =
        std::fs::read(component_path).with_context(|| format!("read {component_path}"))?;
    let mut files = Vec::new();
    if let Ok(bytes) = std::fs::read(folder.join(SETTINGS_SCHEMA_MEMBER)) {
        files.push((SETTINGS_SCHEMA_MEMBER.to_owned(), bytes));
    }
    for sub in ["locales", "page", "panel"] {
        collect(&folder, &folder.join(sub), &mut files)?;
    }
    let key = option(args, "--key")
        .map(|path| {
            let pem = std::fs::read_to_string(path).with_context(|| format!("read {path}"))?;
            load_signing_key_pem(&pem)
        })
        .transpose()?;
    if key.is_none() {
        eprintln!("!! no --key: the package is unsigned and loads only in development mode");
    }
    let bytes = package_plugin(&manifest, &component, &files, key.as_ref())?;
    std::fs::write(&out, bytes).with_context(|| format!("write {}", out.display()))?;
    println!("{}", out.display());
    Ok(())
}

/// Every file below `dir`, named relative to `root` with `/`.
fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            bail!(
                "{} is a link; a package holds files only",
                entry.path().display()
            );
        }
        if kind.is_dir() {
            collect(root, &entry.path(), files)?;
        } else {
            let relative = entry
                .path()
                .strip_prefix(root)
                .context("a member below the plugin folder")?
                .components()
                .map(|part| part.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            files.push((relative, std::fs::read(entry.path())?));
        }
    }
    Ok(())
}

fn trusted(args: &[String]) -> Result<Vec<(String, String)>> {
    args.windows(2)
        .filter(|pair| pair[0] == "--trust")
        .map(|pair| {
            pair[1]
                .split_once('=')
                .map(|(id, key)| (id.to_owned(), key.to_owned()))
                .context("--trust takes <key_id>=<base64 public key>")
        })
        .collect()
}

fn verify(args: &[String]) -> Result<()> {
    let path = args.first().context("verify needs the package")?;
    let bytes = std::fs::read(path).with_context(|| format!("read {path}"))?;
    let dev = args.iter().any(|arg| arg == "--dev");
    match verify_package(&bytes, &trusted(args)?, dev) {
        Ok(package) => {
            println!(
                "{} {} verifies ({} files, {})",
                package.manifest.id,
                package.manifest.version,
                package.files.len(),
                if package.signature.is_some() {
                    "signed"
                } else {
                    "unsigned"
                }
            );
            Ok(())
        }
        Err(error) => bail!("{}: {error:#}", error.code()),
    }
}

fn conformance(args: &[String]) -> Result<()> {
    let path = args.first().context("conformance needs the package")?;
    let bytes = std::fs::read(path).with_context(|| format!("read {path}"))?;
    let verifier = PluginVerifier::with_roots(args.iter().any(|arg| arg == "--dev"))?;
    for (key_id, key) in trusted(args)? {
        verifier.trust_key_base64(key_id, &key)?;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .build()
        .context("start the async runtime")?;
    let report = runtime.block_on(run_conformance(&bytes, &verifier));
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.passed {
        std::process::exit(1);
    }
    Ok(())
}

/// Builds and signs the index of the packages in a folder (RB-091-01).
fn index(args: &[String]) -> Result<()> {
    let folder = PathBuf::from(args.first().context("index needs the folder of packages")?);
    let out = PathBuf::from(option(args, "--out").context("index needs --out")?);
    let key_path = option(args, "--key").context("index needs --key <private.pem>")?;
    let key_id = option(args, "--key-id").context("index needs --key-id")?;
    let sequence: u64 = option(args, "--sequence")
        .context("index needs --sequence")?
        .parse()
        .context("--sequence is a whole number")?;
    let days: i64 = option(args, "--validity-days")
        .map(str::parse)
        .transpose()
        .context("--validity-days is a whole number")?
        .unwrap_or(DEFAULT_VALIDITY_DAYS);
    let base = option(args, "--base-url").map(|base| base.trim_end_matches('/'));
    if let Some(base) = base
        && !base.starts_with("https://")
    {
        bail!("--base-url must be an https:// address");
    }
    let pem = std::fs::read_to_string(key_path).with_context(|| format!("read {key_path}"))?;
    let key = load_signing_key_pem(&pem)?;
    let verifier = PluginVerifier::with_roots(false)?;
    for (trusted_id, public) in trusted(args)? {
        verifier.trust_key_base64(trusted_id, &public)?;
    }
    let categories: serde_json::Map<String, serde_json::Value> = match option(args, "--categories")
    {
        Some(path) => serde_json::from_str(
            &std::fs::read_to_string(path).with_context(|| format!("read {path}"))?,
        )
        .with_context(|| format!("{path} is no JSON object"))?,
        None => serde_json::Map::new(),
    };
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&folder)
        .with_context(|| format!("read {}", folder.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == PACKAGE_EXTENSION))
        .collect();
    paths.sort();
    let mut plugins = Vec::new();
    for path in &paths {
        let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
        let package = verifier
            .verify_bytes(&bytes)
            .map_err(|error| anyhow::anyhow!("{}: {}: {error:#}", path.display(), error.code()))?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("a package file name in UTF-8")?;
        let url = base.map_or_else(|| name.to_owned(), |base| format!("{base}/{name}"));
        let mut entry = describe_package(&bytes, &package, &url, None)?;
        entry.category = categories
            .get(&entry.id)
            .and_then(|plugin| plugin.get("category"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned);
        plugins.push(entry);
    }
    if let Some(previous) = option(args, "--previous") {
        let bytes = std::fs::read(previous).with_context(|| format!("read {previous}"))?;
        let signers = IndexSigners::Pinned(key.verifying_key());
        // The signature only: the previous index may be past its expiry (a release more than
        // the index's validity after the last one), and its entries are what it carries.
        let old = verify_index_signature(&bytes, &signers)
            .map_err(|error| anyhow::anyhow!("{previous}: {}: {error}", error.code()))?;
        plugins = carry_over(&old.plugins, plugins);
    }
    let issued_at = time::OffsetDateTime::now_utc();
    let index = PluginIndex {
        schema_version: PLUGIN_INDEX_SCHEMA_VERSION,
        sequence,
        issued_at,
        expires_at: issued_at + time::SignedDuration::days(days),
        plugins,
    };
    let bytes = sign_index(key_id, &key, &index)?;
    std::fs::write(&out, bytes).with_context(|| format!("write {}", out.display()))?;
    println!(
        "{}: {} plugins, sequence {sequence}, expires {}",
        out.display(),
        index.plugins.len(),
        rb_plugin_host::utc::rfc3339(index.expires_at)
    );
    Ok(())
}
