//! Every SDK template fits the host it is made for: its manifest parses as the host parses one,
//! names its own world, its English locale passes the locale check and its `wit/` is the contract
//! byte for byte. Whether a template compiles is `scripts/check-sdk-templates.sh --build`'s
//! question; this one runs with the host's own tests.

use std::path::{Path, PathBuf};

use rb_plugin_host::{check_locale, parse_manifest};
use rb_plugin_types::PluginKind;

fn template(kind: PluginKind) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../sdk/templates")
        .join(kind.template())
}

#[test]
fn every_template_parses_as_its_world_with_the_contract() {
    for kind in PluginKind::ALL {
        let folder = template(kind);
        let manifest = std::fs::read(folder.join("manifest.toml")).expect("a manifest");
        let manifest = parse_manifest(&manifest).unwrap_or_else(|error| {
            panic!("{}: {error:#}", folder.display());
        });
        assert_eq!(manifest.kinds, vec![kind], "{}", folder.display());
        assert_eq!(manifest.api_version, rb_plugin_api::API_VERSION);
        let locale = std::fs::read(folder.join("locales/en.json")).expect("an English locale");
        check_locale(&manifest.id, &locale).expect("the locale passes");
        let wit = std::fs::read_to_string(folder.join("wit/rbrowser-plugin.wit")).expect("wit");
        assert_eq!(
            wit,
            rb_plugin_api::WIT,
            "{}: wit/ is the contract",
            folder.display()
        );
        let guest = std::fs::read_to_string(folder.join("src/guest.rs")).expect("a guest");
        assert!(
            guest.contains(&format!("world: \"{}\"", kind.world())),
            "{}: the guest generates its world",
            folder.display()
        );
    }
}
