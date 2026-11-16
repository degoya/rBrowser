use std::collections::BTreeSet;

use rb_config::trust::{BlockedItem, ProtectedKind};
use rb_plugin_host::Grants;
use rb_plugin_types::Capability;

use super::{capabilities, declared, details, details_of};

fn grants(granted: &[Capability]) -> Grants {
    let mut grants = Grants::default();
    grants.granted = granted.iter().copied().collect::<BTreeSet<_>>();
    grants
}

#[test]
fn evaluate_and_page_controlling_cdp_domains_are_the_risky_parts() {
    let mut asked = grants(&[
        Capability::Evaluate,
        Capability::Cdp,
        Capability::Http,
        Capability::ProjectRead,
        Capability::Sidecar,
    ]);
    asked.cdp_domains = vec!["CSS".to_owned(), "Runtime".to_owned(), "Network".to_owned()];
    asked.http_rate_per_second = 12;
    asked.project_globs = vec!["src/**/*.css".to_owned()];
    asked.sidecar_methods = vec!["lighthouse.run".to_owned()];
    let shown = details(&asked);
    assert!(shown.evaluate);
    assert_eq!(shown.cdp_domains, ["CSS", "Runtime", "Network"]);
    assert_eq!(shown.http_rate_per_second, Some(12));
    assert_eq!(shown.project_globs, ["src/**/*.css"]);
    assert_eq!(shown.sidecar_methods, ["lighthouse.run"]);
    assert_eq!(
        shown.risky,
        ["evaluate", "cdp:Runtime", "cdp:Network", "sidecar"]
    );
}

#[test]
fn project_write_is_risky_and_its_globs_are_shown() {
    let mut asked = grants(&[Capability::ProjectWrite]);
    asked.write_globs = vec!["**/*.todo".to_owned()];
    let shown = details(&asked);
    assert_eq!(shown.write_globs, ["**/*.todo"]);
    assert_eq!(shown.risky, ["project-write"]);
}

#[test]
fn settings_without_their_capability_are_not_shown() {
    let mut asked = grants(&[Capability::Views]);
    asked.cdp_domains = vec!["Page".to_owned()];
    asked.write_globs = vec!["**/*.todo".to_owned()];
    let shown = details(&asked);
    assert!(!shown.evaluate);
    assert!(shown.cdp_domains.is_empty());
    assert!(shown.write_globs.is_empty());
    assert_eq!(shown.http_rate_per_second, None, "no http, no rate");
    assert!(shown.risky.is_empty());
}

#[test]
fn the_dialog_lists_a_project_plugins_grants_by_its_path_only() {
    let root = tempfile::tempdir().expect("temp dir");
    std::fs::create_dir_all(root.path().join(".rbrowser/plugins")).expect("dir");
    std::fs::write(root.path().join(".rbrowser/plugins/x.rbplugin"), b"no zip").expect("file");
    let item = |kind, path: &str| BlockedItem {
        kind,
        path: path.to_owned(),
    };
    let listed = declared(
        root.path(),
        &[
            item(ProtectedKind::Plugins, ".rbrowser/plugins/x.rbplugin"),
            item(ProtectedKind::Plugins, ".rbrowser/plugins/../../y.rbplugin"),
            item(ProtectedKind::Plugins, ".rbrowser/plugins/readme.md"),
            item(ProtectedKind::Flows, ".rbrowser/flows/a.rbplugin"),
        ],
    );
    // A package that does not read declares nothing and has no details; only real plugin
    // packages are named.
    assert_eq!(listed.len(), 1);
    let capabilities = capabilities(&listed);
    assert_eq!(
        capabilities[".rbrowser/plugins/x.rbplugin"],
        Vec::<String>::new()
    );
    assert!(details_of(&listed).is_empty());
}

#[test]
fn network_rules_and_console_logs_are_risky_too() {
    // Rules mock, block or rewrite every request of the views; the console feed carries what the
    // pages log, tokens included (review finding L3 of Charge 7).
    let shown = details(&grants(&[
        Capability::NetworkRules,
        Capability::ConsoleLogs,
        Capability::Views,
    ]));
    assert_eq!(shown.risky, ["network-rules", "console-logs"]);
    assert!(details(&grants(&[Capability::Views])).risky.is_empty());
}

#[test]
fn the_sidecar_is_risky_too() {
    // Lighthouse and Playwright run in Node, outside the sandbox; Lighthouse loads and reports
    // what it is given (security review RB-090-06, finding 4; RB-090-21).
    let shown = details(&grants(&[Capability::Sidecar, Capability::Views]));
    assert_eq!(shown.risky, ["sidecar"]);
}
