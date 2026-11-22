use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use rb_plugin_host::Sources;

use super::protected_keys;

/// The API client's shipped manifest and settings schema: a real package's two members.
fn shipped(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/api-client")
        .join(file);
    std::fs::read_to_string(&path).expect("the API client's files are there")
}

/// A manifest of the foreign plugin `id`, with or without settings: the API client's shipped
/// header (versions, kinds, key) under another id, without its contributions.
fn manifest(id: &str, settings: bool) -> String {
    let shipped = shipped("manifest.toml");
    let header: Vec<String> = shipped
        .lines()
        .take_while(|line| !line.starts_with('['))
        .filter(|line| !line.starts_with('#') && !line.starts_with("mcp_prefix"))
        .map(|line| {
            if line.starts_with("id = ") {
                format!("id = \"{id}\"")
            } else {
                line.to_owned()
            }
        })
        .collect();
    let settings = if settings {
        "\n[settings]\nschema = \"settings.schema.json\"\n"
    } else {
        ""
    };
    format!(
        "{}\n\n[locales]\nen = \"locales/en.json\"\n{settings}",
        header.join("\n")
    )
}

/// A package at `path` with `members`; not signed, never loaded - only read.
fn package(path: &Path, members: &[(&str, &[u8])]) {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).expect("the folder");
    }
    let mut writer = zip::ZipWriter::new(std::fs::File::create(path).expect("the package"));
    for (name, bytes) in members {
        writer
            .start_file(*name, zip::write::SimpleFileOptions::default())
            .expect("a member");
        writer.write_all(bytes).expect("written");
    }
    writer.finish().expect("finished");
}

fn user_only(folder: &Path) -> Sources {
    Sources {
        user: Some(folder.to_path_buf()),
        ..Sources::default()
    }
}

fn keys(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|key| (*key).to_owned()).collect()
}

/// RB-094-31: a plugin no registry knows - installed by the person, not loaded - has its
/// `x-protected` keys guarded from its package alone; its other keys stay open.
#[test]
fn an_unloaded_foreign_plugins_protected_keys_are_read_from_its_package() {
    let dir = tempfile::tempdir().expect("a temp folder");
    let manifest = manifest("acme-hosts", true);
    let schema = shipped("settings.schema.json");
    package(
        &dir.path().join("acme-hosts.rbplugin"),
        &[
            ("manifest.toml", manifest.as_bytes()),
            ("settings.schema.json", schema.as_bytes()),
        ],
    );
    let guarded = protected_keys(&user_only(dir.path()));
    assert_eq!(
        guarded,
        keys(&["acme-hosts.allowedHosts", "acme-hosts.secrets"])
    );
    assert!(!guarded.contains("acme-hosts.historySize"));
}

/// The bundled, the person's and the project's folders are all read - the project's whether
/// the workspace is trusted or not: what is read only adds refusals.
#[test]
fn every_plugin_folder_is_read() {
    let dir = tempfile::tempdir().expect("a temp folder");
    let schema = shipped("settings.schema.json");
    let project_root: PathBuf = dir.path().join("project");
    for (folder, id) in [
        (dir.path().join("bundled"), "one-plugin"),
        (dir.path().join("user"), "two-plugin"),
        (
            project_root.join(".rbrowser").join("plugins"),
            "three-plugin",
        ),
    ] {
        let manifest = manifest(id, true);
        package(
            &folder.join(format!("{id}.rbplugin")),
            &[
                ("manifest.toml", manifest.as_bytes()),
                ("settings.schema.json", schema.as_bytes()),
            ],
        );
    }
    let sources = Sources {
        bundled: Some(dir.path().join("bundled")),
        user: Some(dir.path().join("user")),
        project_root: Some(project_root),
        ..Sources::default()
    };
    let guarded = protected_keys(&sources);
    for id in ["one-plugin", "two-plugin", "three-plugin"] {
        assert!(guarded.contains(&format!("{id}.allowedHosts")), "{id}");
        assert!(guarded.contains(&format!("{id}.secrets")), "{id}");
    }
    assert_eq!(guarded.len(), 6, "{guarded:?}");
}

/// A package whose settings schema does not read - no JSON, or missing - guards its whole
/// prefix: the keys it would protect are not known.
#[test]
fn a_package_whose_schema_does_not_read_is_guarded_whole() {
    let dir = tempfile::tempdir().expect("a temp folder");
    let first = manifest("acme-hosts", true);
    let second = manifest("acme-proxy", true);
    package(
        &dir.path().join("acme-hosts.rbplugin"),
        &[
            ("manifest.toml", first.as_bytes()),
            ("settings.schema.json", b"{ no json".as_slice()),
        ],
    );
    package(
        &dir.path().join("acme-proxy.rbplugin"),
        &[("manifest.toml", second.as_bytes())],
    );
    assert_eq!(
        protected_keys(&user_only(dir.path())),
        keys(&["acme-hosts", "acme-proxy"])
    );
}

/// A package whose manifest does not read guards the id its file name names - `<id>.rbplugin`
/// and `<id>-<version>.rbplugin` - never a name that is no plugin id or a reserved one.
#[test]
fn a_package_whose_manifest_does_not_read_is_guarded_by_its_file_name() {
    let dir = tempfile::tempdir().expect("a temp folder");
    for name in [
        "acme-hosts.rbplugin",
        "acme-proxy-1.2.0.rbplugin",
        "Not An Id.rbplugin",
        "plugins.rbplugin",
    ] {
        std::fs::write(dir.path().join(name), b"no zip at all").expect("written");
    }
    package(
        &dir.path().join("acme-shot.rbplugin"),
        &[("manifest.toml", b"id = ".as_slice())],
    );
    assert_eq!(
        protected_keys(&user_only(dir.path())),
        keys(&["acme-hosts", "acme-proxy", "acme-shot"])
    );
}

/// A plugin whose manifest declares no settings guards nothing; a folder without packages
/// neither.
#[test]
fn a_plugin_without_settings_guards_nothing() {
    let dir = tempfile::tempdir().expect("a temp folder");
    let manifest = manifest("acme-quiet", false);
    assert!(
        !manifest.contains("[settings]"),
        "the test manifest has no settings"
    );
    package(
        &dir.path().join("acme-quiet.rbplugin"),
        &[("manifest.toml", manifest.as_bytes())],
    );
    assert!(protected_keys(&user_only(dir.path())).is_empty());
    assert!(protected_keys(&user_only(&dir.path().join("missing"))).is_empty());
}
