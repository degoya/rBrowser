use std::{collections::BTreeSet, path::Path};

use rb_plugin_types::{Capability, codes};

use super::{LoadContext, PluginSource, Sources, discover, load_all, project_plugins_dir};
use crate::sinks::{DenyAllTrust, NoLocales, NoSettings, WorkspaceTrust};
use crate::test_support::{KEY_ID, ManifestBuilder, signing_key};
use crate::testbed::Bed;
use crate::{PluginOptions, PluginVerifier, Registry};

struct TrustEverything;

impl WorkspaceTrust for TrustEverything {
    fn is_trusted(&self, _project_root: &Path) -> bool {
        true
    }
}

fn verifier() -> PluginVerifier {
    let verifier = PluginVerifier::new(false);
    verifier
        .trust_key(KEY_ID.to_owned(), signing_key(1).verifying_key())
        .expect("trust");
    verifier
}

fn put_project_plugin(root: &Path, builder: &ManifestBuilder) {
    let folder = project_plugins_dir(root);
    std::fs::create_dir_all(&folder).expect("plugins folder");
    let package = builder.package(&Bed::feature().build(), &signing_key(1), true, &[]);
    std::fs::write(folder.join(format!("{}.rbplugin", builder.id)), package).expect("write");
}

fn context<'a>(verifier: &'a PluginVerifier, trust: &'a dyn WorkspaceTrust) -> LoadContext<'a> {
    LoadContext {
        verifier,
        trust,
        locales: &NoLocales,
        settings: &NoSettings,
        options: PluginOptions::default(),
        disabled: BTreeSet::new(),
    }
}

#[tokio::test]
async fn project_plugins_do_not_load_without_workspace_trust() {
    let project = tempfile::tempdir().expect("tempdir");
    put_project_plugin(
        project.path(),
        &ManifestBuilder::default().capability("views = true"),
    );
    let sources = Sources {
        project_root: Some(project.path().to_path_buf()),
        ..Sources::default()
    };
    let verifier = verifier();
    let mut registry = Registry::new();
    let report = load_all(&sources, &context(&verifier, &DenyAllTrust), &mut registry).await;
    assert!(report.loaded.is_empty());
    assert_eq!(report.refused.len(), 1);
    let refused = &report.refused[0];
    assert_eq!(refused.source, PluginSource::Project);
    assert_eq!(refused.failure.code(), Some(codes::UNTRUSTED_WORKSPACE));
    assert!(
        refused.claims.is_none(),
        "no text of an untrusted workspace's package is taken"
    );
    assert_eq!(
        refused.capabilities,
        vec![Capability::Views],
        "shown to the trust prompt"
    );
    assert!(registry.get("sample").is_none());

    let report = load_all(
        &sources,
        &context(&verifier, &TrustEverything),
        &mut registry,
    )
    .await;
    assert_eq!(
        report.loaded.len(),
        1,
        "trusted, it loads: {:?}",
        report.refused
    );
    assert!(registry.get("sample").is_some());
}

#[tokio::test]
async fn the_second_plugin_with_a_taken_prefix_is_refused_and_the_first_kept() {
    let user = tempfile::tempdir().expect("tempdir");
    for id in ["alpha", "beta"] {
        let builder = ManifestBuilder::named(id, "shared");
        let package = builder.package(&Bed::feature().build(), &signing_key(1), true, &[]);
        std::fs::write(user.path().join(format!("{id}.rbplugin")), package).expect("write");
    }
    let sources = Sources {
        user: Some(user.path().to_path_buf()),
        ..Sources::default()
    };
    let verifier = verifier();
    let mut registry = Registry::new();
    let report = load_all(&sources, &context(&verifier, &DenyAllTrust), &mut registry).await;
    assert_eq!(report.loaded.len(), 1);
    assert_eq!(report.loaded[0].plugin.manifest().id, "alpha");
    let refused = &report.refused[0].failure;
    assert_eq!(refused.code(), Some(codes::MCP_PREFIX_TAKEN));
    assert_eq!(refused.param("other"), Some("alpha"));
}

#[tokio::test]
async fn a_disabled_plugin_is_skipped_and_an_unknown_key_reported_with_its_fingerprint() {
    let user = tempfile::tempdir().expect("tempdir");
    let builder = ManifestBuilder::default();
    let package = builder.package(&Bed::feature().build(), &signing_key(9), true, &[]);
    std::fs::write(user.path().join("sample.rbplugin"), package).expect("write");
    let sources = Sources {
        user: Some(user.path().to_path_buf()),
        ..Sources::default()
    };
    // A verifier that trusts no key yet: the package's key is a person's decision.
    let verifier = PluginVerifier::new(false);
    let mut registry = Registry::new();
    let report = load_all(&sources, &context(&verifier, &DenyAllTrust), &mut registry).await;
    let refused = &report.refused[0];
    assert_eq!(refused.failure.code(), Some(codes::KEY_UNTRUSTED));
    assert!(
        refused
            .untrusted_key
            .as_ref()
            .is_some_and(|(id, _)| id == KEY_ID)
    );

    let mut disabled = context(&verifier, &DenyAllTrust);
    disabled.disabled.insert("sample".to_owned());
    verifier
        .trust_key(KEY_ID.to_owned(), signing_key(9).verifying_key())
        .expect("confirmed by a person");
    let report = load_all(&sources, &disabled, &mut registry).await;
    assert!(report.loaded.is_empty() && report.refused.is_empty());
    // Skipped, but known: Settings > Plugins lists it as disabled (RB-080-51).
    assert_eq!(report.disabled.len(), 1);
    let skipped = &report.disabled[0];
    assert_eq!(skipped.manifest.id, "sample");
    assert_eq!(skipped.source, PluginSource::User);
    assert!(skipped.signed);
    assert!(registry.get("sample").is_none(), "not admitted");
}

#[tokio::test]
async fn the_report_says_whether_a_package_is_signed() {
    let user = tempfile::tempdir().expect("tempdir");
    let signed = ManifestBuilder::named("alpha", "shared");
    let package = signed.package(&Bed::feature().build(), &signing_key(1), true, &[]);
    std::fs::write(user.path().join("alpha.rbplugin"), package).expect("write");
    // The same MCP prefix: refused after its verification, which it passed unsigned.
    let unsigned = ManifestBuilder::named("beta", "shared");
    let package = unsigned.package(&Bed::feature().build(), &signing_key(1), false, &[]);
    std::fs::write(user.path().join("beta.rbplugin"), package).expect("write");
    std::fs::write(user.path().join("gamma.rbplugin"), b"no zip").expect("write");
    let sources = Sources {
        user: Some(user.path().to_path_buf()),
        ..Sources::default()
    };
    // Development mode: an unsigned package verifies.
    let verifier = PluginVerifier::new(true);
    verifier
        .trust_key(KEY_ID.to_owned(), signing_key(1).verifying_key())
        .expect("trust");
    let mut registry = Registry::new();
    let report = load_all(&sources, &context(&verifier, &DenyAllTrust), &mut registry).await;
    assert_eq!(report.loaded.len(), 1, "{:?}", report.refused);
    assert!(report.loaded[0].signed);
    let by_name = |name: &str| {
        report
            .refused
            .iter()
            .find(|refused| refused.path.ends_with(name))
            .expect("refused")
    };
    assert_eq!(by_name("beta.rbplugin").signed, Some(false));
    assert_eq!(by_name("gamma.rbplugin").signed, None, "never verified");
    // What a refused package says of itself is read once, at the load (RB-080-51).
    let claims = by_name("beta.rbplugin").claims.as_ref().expect("reads");
    assert_eq!(claims.id, "beta");
    assert!(by_name("gamma.rbplugin").claims.is_none(), "no zip");
}

#[test]
fn discovery_reads_bundled_then_user_then_project_packages_only() {
    let bundled = tempfile::tempdir().expect("tempdir");
    let project = tempfile::tempdir().expect("tempdir");
    std::fs::write(bundled.path().join("a.rbplugin"), b"x").expect("write");
    std::fs::write(bundled.path().join("notes.txt"), b"x").expect("write");
    std::fs::create_dir_all(project_plugins_dir(project.path())).expect("dir");
    std::fs::write(project_plugins_dir(project.path()).join("b.rbplugin"), b"x").expect("write");
    let found = discover(&Sources {
        bundled: Some(bundled.path().to_path_buf()),
        user: None,
        project_root: Some(project.path().to_path_buf()),
    });
    let sources: Vec<PluginSource> = found.iter().map(|(source, _)| *source).collect();
    assert_eq!(sources, vec![PluginSource::Bundled, PluginSource::Project]);
}

#[tokio::test]
async fn a_loaded_plugin_keeps_its_page_and_panel_files_for_the_runtime() {
    let user = tempfile::tempdir().expect("tempdir");
    let builder = ManifestBuilder::default().tables(
        "\n[[page_scripts]]\npath = \"page/a.js\"\n\
         \n[[panels]]\nid = \"main\"\ntitle = \"plugin.sample.panels.main\"\nentry = \"panel/index.html\"\n",
    );
    let extra = vec![
        ("page/a.js".to_owned(), b"rbPlugin.emit(1);".to_vec()),
        ("panel/index.html".to_owned(), b"<p>hi</p>".to_vec()),
        ("panel/app.js".to_owned(), b"1;".to_vec()),
    ];
    let package = builder.package(&Bed::feature().build(), &signing_key(1), true, &extra);
    std::fs::write(user.path().join("sample.rbplugin"), package).expect("write");
    let sources = Sources {
        user: Some(user.path().to_path_buf()),
        ..Sources::default()
    };
    let verifier = verifier();
    let mut registry = Registry::new();
    let report = load_all(&sources, &context(&verifier, &DenyAllTrust), &mut registry).await;
    let files = &report.loaded[0].ui_files;
    let names: Vec<&str> = files.keys().map(String::as_str).collect();
    assert_eq!(names, ["page/a.js", "panel/app.js", "panel/index.html"]);
    assert_eq!(files["page/a.js"], b"rbPlugin.emit(1);");
}

#[test]
fn the_declared_grants_are_read_without_verifying_the_package() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manifest = ManifestBuilder::default()
        .capability("evaluate = true")
        .capability("cdp = { domains = [\"DOM\", \"CSS\"] }")
        .capability("http = { rate_limit_per_second = 9 }")
        .capability("project-read = { globs = [\"src/**/*.css\"] }")
        .capability("sidecar = { methods = [\"lighthouse.run\"] }")
        .text(&signing_key(1));
    let path = dir.path().join("x.rbplugin");
    let package = crate::test_support::zip_package(&[("manifest.toml", manifest.as_bytes())]);
    std::fs::write(&path, package).expect("package");

    let claims = super::declared_manifest(&path).expect("reads");
    assert_eq!(claims.id, "sample");
    let grants = super::declared_grants(&path).expect("reads");
    assert!(grants.has(Capability::Evaluate));
    assert_eq!(grants.cdp_domains, ["DOM", "CSS"]);
    assert_eq!(grants.http_rate_per_second, 9);
    assert_eq!(grants.project_globs, ["src/**/*.css"]);
    assert_eq!(grants.sidecar_methods, ["lighthouse.run"]);
    assert!(super::declared_capabilities(&path).contains(&Capability::Cdp));

    std::fs::write(&path, b"no zip").expect("broken");
    assert!(super::declared_grants(&path).is_none());
    assert!(super::declared_manifest(&path).is_none());
    assert!(super::declared_capabilities(&path).is_empty());
}
