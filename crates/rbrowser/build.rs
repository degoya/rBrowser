use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn main() {
    println!("cargo:rerun-if-changed=windows-resource.rc");
    println!("cargo:rerun-if-changed=resources/windows-long-path.manifest");
    println!("cargo:rerun-if-changed=resources/rbrowser.ico");
    stamp_build();
    embed_cef_pin();
    embed_sync_agent();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        write_version_resource("rBrowser", "rbrowser", "rbrowser.exe");
        embed_resource::compile("windows-resource.rc", embed_resource::NONE)
            .manifest_required()
            .expect("failed to compile Windows resources");
    }
}

/// Compiles the commit and the build time into the binary (`rbrowser --version`, later the About
/// page of the settings and the diagnostics bundle).
///
/// A package build may export both (`RB_BUILD_COMMIT`, `RB_BUILD_TIME`), and whatever form it
/// gives is taken verbatim. A development build works them out here: eight characters of the
/// commit, `-dirty` when the tree differs from it, `unknown` without git.
fn stamp_build() {
    println!("cargo:rerun-if-env-changed=RB_BUILD_COMMIT");
    println!("cargo:rerun-if-env-changed=RB_BUILD_TIME");
    let commit = exported("RB_BUILD_COMMIT").unwrap_or_else(git_commit);
    let built = exported("RB_BUILD_TIME").unwrap_or_else(utc_now);
    println!("cargo:rustc-env=RB_BUILD_COMMIT={commit}");
    println!("cargo:rustc-env=RB_BUILD_TIME={built}");
}

/// Embeds the sync agent (RB-020-01), the page API (RB-020-08), the portal agent (RB-080-05)
/// and the Framework DevTools scripts (RB-080-24): the bundles `npm run build` in `agent/`
/// writes (`scripts/check.sh --preflight` runs it), copied to `OUT_DIR` for
/// `sync_agent::AGENT_SOURCE`, `sync_agent::page::PAGE_API_SOURCE` and
/// `sync_agent::framework::{PROBE_SOURCE, REACT_SOURCE, VUE_SOURCE}`. Without a build the copies
/// are empty and the views get none of them, so the Rust build never needs npm; a warning says
/// so.
fn embed_sync_agent() {
    let dist = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("../../agent/dist");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    for (file, what) in [
        ("rb-agent.js", "views are not synced"),
        ("rb-page-api.js", "pages get no window.__rbrowser"),
        // The portal's devices (RB-080-05).
        ("rb-portal-agent.js", "portal devices are not synced"),
        // The Framework DevTools (RB-080-24): the probe and the two backends.
        ("rb-framework-probe.js", "no Vue or React is detected"),
        ("rb-react-devtools.js", "no React DevTools"),
        ("rb-vue-devtools.js", "no Vue DevTools"),
    ] {
        let bundle = dist.join(file);
        println!("cargo:rerun-if-changed={}", bundle.display());
        let text = std::fs::read_to_string(&bundle).unwrap_or_else(|_| {
            println!(
                "cargo:warning=no bundle at {} (npm run build in agent/): {what}",
                bundle.display()
            );
            String::new()
        });
        std::fs::write(out.join(file), text).expect("OUT_DIR is writable");
    }
}

/// Embeds the pinned CEF build for the age notice (RB-010-10): its release date and its
/// version, read from the one source, `scripts/lib/tool-versions.sh` (`RB_CEF_RELEASE_DATE`,
/// `RB_CEF_ARCHIVE`). A missing pin embeds an empty value, which raises no notice.
fn embed_cef_pin() {
    let pins = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory"))
        .join("../../scripts/lib/tool-versions.sh");
    println!("cargo:rerun-if-changed={}", pins.display());
    let text = std::fs::read_to_string(&pins).unwrap_or_default();
    let date = pin(&text, "RB_CEF_RELEASE_DATE").unwrap_or_default();
    let version = pin(&text, "RB_CEF_ARCHIVE")
        .and_then(|archive| {
            let rest = archive.strip_prefix("cef_binary_")?;
            Some(rest.split_once("_windows64")?.0.to_owned())
        })
        .unwrap_or_default();
    if date.is_empty() || version.is_empty() {
        println!(
            "cargo:warning=no CEF pin in {}: the CEF age notice is off",
            pins.display()
        );
    }
    println!("cargo:rustc-env=RB_CEF_RELEASE_DATE={date}");
    println!("cargo:rustc-env=RB_CEF_BUILD_VERSION={version}");
}

/// The value of the shell assignment `NAME="value"` at the start of a line of `text`.
fn pin(text: &str, name: &str) -> Option<String> {
    let prefix = format!("{name}=\"");
    text.lines().find_map(|line| {
        let value = line.strip_prefix(&prefix)?.strip_suffix('"')?;
        Some(value.to_owned())
    })
}

fn exported(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.trim().is_empty())
}

fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn git_commit() -> String {
    // Without these the script would run once and a development binary would name the commit
    // it was first built at for good. The index stands in for the working tree: `-dirty` is as
    // current as the last `git add`, which is as close as a build script can watch cheaply.
    let mut watched = vec![
        "HEAD".to_owned(),
        "index".to_owned(),
        "packed-refs".to_owned(),
    ];
    watched.extend(git(&["symbolic-ref", "-q", "HEAD"]));
    for name in watched {
        if let Some(path) = git(&["rev-parse", "--git-path", &name])
            && Path::new(&path).exists()
        {
            println!("cargo:rerun-if-changed={path}");
        }
    }
    let Some(commit) = git(&["rev-parse", "--short=8", "HEAD"]) else {
        return "unknown".to_owned();
    };
    // Exit code 1 is "differs"; anything else is git failing, which is not a dirty tree.
    let dirty = Command::new("git")
        .args(["diff", "--quiet", "HEAD", "--"])
        .status()
        .is_ok_and(|status| status.code() == Some(1));
    if dirty {
        format!("{commit}-dirty")
    } else {
        commit
    }
}

/// The current time as `YYYY-MM-DDTHH:MM:SSZ`, without a date crate in the build graph.
fn utc_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's `civil_from_days`).
    let z = days + 719_468;
    let era = z / 146_097;
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Writes the Windows version resource `rbrowser-version.rc2` into `OUT_DIR`, where
/// `windows-resource.rc` includes it. Run only for a Windows target.
fn write_version_resource(description: &str, internal_name: &str, original_filename: &str) {
    let major = env::var("CARGO_PKG_VERSION_MAJOR").expect("package major version");
    let minor = env::var("CARGO_PKG_VERSION_MINOR").expect("package minor version");
    let patch = env::var("CARGO_PKG_VERSION_PATCH").expect("package patch version");
    let version = env::var("CARGO_PKG_VERSION").expect("package version");
    let resource = format!(
        r#"1 VERSIONINFO
FILEVERSION {major},{minor},{patch},0
PRODUCTVERSION {major},{minor},{patch},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904B0"
        BEGIN
            VALUE "CompanyName", "DEGOYA.digital\0"
            VALUE "FileDescription", "{description}\0"
            VALUE "FileVersion", "{version}\0"
            VALUE "InternalName", "{internal_name}\0"
            VALUE "LegalCopyright", "Copyright (c) DEGOYA.digital\0"
            VALUE "OriginalFilename", "{original_filename}\0"
            VALUE "ProductName", "rBrowser\0"
            VALUE "ProductVersion", "{version}\0"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    );
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("build output directory"))
        .join("rbrowser-version.rc2");
    std::fs::write(output, resource).expect("write Windows version resource");
}
