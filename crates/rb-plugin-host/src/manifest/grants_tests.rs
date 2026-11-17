use rb_plugin_types::Capability;

use super::{Grants, PLUGIN_SIDECAR_NAMESPACES};

fn parse(table: &str) -> anyhow::Result<Grants> {
    Grants::parse(&toml::from_str(table).expect("toml"))
}

#[test]
fn sidecar_grants_exactly_the_listed_methods() {
    let grants = parse("sidecar = { methods = [\"lighthouse.run\"] }").expect("grants");
    assert!(grants.has(Capability::Sidecar));
    assert_eq!(grants.sidecar_methods, vec!["lighthouse.run".to_owned()]);
    assert!(grants.sidecar_allows("lighthouse.run"));
    assert!(!grants.sidecar_allows("lighthouse.audit"));
    assert!(!grants.sidecar_allows("lighthouse"));
}

#[test]
fn sidecar_needs_its_methods_and_never_the_cores_own() {
    for table in [
        "sidecar = true",
        "sidecar = {}",
        "sidecar = { methods = [] }",
        "sidecar = { methods = [\"run\"] }",
        "sidecar = { methods = [\"a.b c\"] }",
        "sidecar = { methods = [\"lighthouse.run\"], other = 1 }",
    ] {
        assert!(parse(table).is_err(), "{table}");
    }
    // The core's own namespaces, and any namespace a later sidecar module adds, until it is
    // listed for plugins.
    for namespace in [
        "engines",
        "flow",
        "flows",
        "playwright",
        "sidecar",
        "webkit",
        "network",
        "future",
    ] {
        let table = format!("sidecar = {{ methods = [\"{namespace}.anything\"] }}");
        assert!(parse(&table).is_err(), "{table}");
    }
}

#[test]
fn sidecar_namespaces_are_an_allowlist() {
    assert_eq!(PLUGIN_SIDECAR_NAMESPACES, ["lighthouse"]);
    for namespace in PLUGIN_SIDECAR_NAMESPACES {
        let table = format!("sidecar = {{ methods = [\"{namespace}.start\"] }}");
        assert!(parse(&table).is_ok(), "{table}");
    }
}

#[test]
fn cdp_domains_are_only_those_of_the_method_allowlist() {
    let grants = parse("cdp = { domains = [\"DOM\", \"Network\", \"Runtime\"] }").expect("grants");
    assert_eq!(grants.cdp_domains, ["DOM", "Network", "Runtime"]);
    for domain in [
        "Browser",
        "Fetch",
        "Input",
        "Security",
        "Storage",
        "Target",
        "Tethering",
        "SystemInfo",
        "IndexedDB",
        "Debugger",
        "HeapProfiler",
        "dom",
    ] {
        let table = format!("cdp = {{ domains = [\"{domain}\"] }}");
        assert!(parse(&table).is_err(), "{table}");
    }
}

#[test]
fn no_sidecar_grant_allows_no_method() {
    let grants = parse("views = true").expect("grants");
    assert!(!grants.sidecar_allows("lighthouse.run"));
}

#[test]
fn the_window_image_is_a_cdp_permission_of_its_own() {
    // Review S2 of RB-080-64: only a plugin whose manifest asks for it may take the view's
    // window image without a clip.
    let plain = parse("cdp = { domains = [\"Page\"] }").expect("grants");
    assert!(!plain.cdp_window_image);
    let window = parse("cdp = { domains = [\"Page\"], window_image = true }").expect("grants");
    assert!(window.cdp_window_image);
    assert_eq!(window.cdp_domains, ["Page"]);
    for table in [
        "cdp = { domains = [\"Page\"], window_image = \"yes\" }",
        "cdp = { domains = [\"DOM\"], window_image = true }",
        "cdp = { domains = [\"Page\"], window = true }",
    ] {
        assert!(parse(table).is_err(), "{table}");
    }
}

#[test]
fn project_write_grants_exactly_its_globs() {
    let grants = parse(
        "project-read = { globs = [\"**/*.todo\", \"docs/*.taskpaper\"] }\n\
         project-write = { globs = [\"**/*.todo\", \"docs/*.taskpaper\"] }",
    )
    .expect("grants");
    assert!(grants.has(Capability::ProjectWrite));
    assert_eq!(grants.write_globs, ["**/*.todo", "docs/*.taskpaper"]);
    assert!(grants.write_glob_matches("TODO.todo"));
    assert!(grants.write_glob_matches("a/b/plan.todo"));
    assert!(grants.write_glob_matches("docs/notes.taskpaper"));
    // `*` stays in its folder (security review RB-110-13, finding 1).
    assert!(!grants.write_glob_matches("docs/sub/notes.taskpaper"));
    assert!(!grants.write_glob_matches("src/main.rs"));
    assert!(!grants.write_glob_matches("plan.todo.bak"));
    // Reading grants no writing.
    let read = parse("project-read = { globs = [\"**/*.todo\"] }").expect("grants");
    assert!(!read.write_glob_matches("TODO.todo"));
}

#[test]
fn project_write_needs_globs_that_name_a_file_type_outside_git_and_the_config() {
    for table in [
        // No globs: nothing would be granted, so the manifest is wrong.
        "project-write = true",
        "project-write = {}",
        "project-write = { globs = [] }",
        "project-write = { globs = [\"*.todo\"], other = 1 }",
        // Every file, or every file of a folder: no file type named.
        "project-write = { globs = [\"**\"] }",
        "project-write = { globs = [\"**/*\"] }",
        "project-write = { globs = [\"src/**\"] }",
        "project-write = { globs = [\"*.*\"] }",
        "project-write = { globs = [\"*.{todo,md}\"] }",
        "project-write = { globs = [\"*.t?do\"] }",
        "project-write = { globs = [\"Makefile\"] }",
        // Only plain todo lists: no type a tool runs or an agent reads as instructions
        // (security review RB-110-13, finding 1).
        "project-write = { globs = [\"**/*.json\"] }",
        "project-write = { globs = [\"**/*.js\"] }",
        "project-write = { globs = [\"*.ps1\"] }",
        "project-write = { globs = [\"**/build.rs\"] }",
        "project-write = { globs = [\".pre-commit-config.yaml\"] }",
        "project-write = { globs = [\"Directory.Build.props\"] }",
        "project-write = { globs = [\"**/*.md\"] }",
        "project-write = { globs = [\"**/*.txt\"] }",
        "project-write = { globs = [\".envrc\"] }",
        "project-write = { globs = [\"**/.env\"] }",
        "project-write = { globs = [\"**/.todo\"] }",
        // Out of the project, or into version control and the project config.
        "project-write = { globs = [\"../*.todo\"] }",
        "project-write = { globs = [\"/etc/*.conf\"] }",
        "project-write = { globs = [\"C:/x/*.todo\"] }",
        "project-write = { globs = [\".git/*.todo\"] }",
        "project-write = { globs = [\"sub/.GIT/hooks/*.sample\"] }",
        "project-write = { globs = [\".rbrowser/*.json\"] }",
        "project-write = { globs = [\".Rbrowser/snippets/*.js\"] }",
        "project-write = { globs = [\"docs\\\\*.todo\"] }",
    ] {
        assert!(parse(table).is_err(), "{table}");
    }
}

#[test]
fn project_write_needs_each_of_its_globs_among_the_read_globs() {
    // Every write would be refused at run time (a write path must match a read glob): refused
    // when the manifest is read instead (code review RB-110-13).
    for table in [
        "project-write = { globs = [\"**/*.todo\"] }",
        "project-read = true\nproject-write = { globs = [\"**/*.todo\"] }",
        "project-read = { globs = [\"docs/*.todo\"] }\nproject-write = { globs = [\"**/*.todo\"] }",
        "project-read = { globs = [\"**/*.todo\"] }\nproject-write = { globs = [\"**/*.todo\", \"*.taskpaper\"] }",
    ] {
        let refused = parse(table).expect_err(table);
        assert!(
            format!("{refused:#}").contains("project-read"),
            "{table}: {refused:#}"
        );
    }
    let both = parse(
        "project-read = { globs = [\"**/*.todo\", \"src/**/*.css\"] }\n\
         project-write = { globs = [\"**/*.todo\"] }",
    )
    .expect("a write glob listed among the read globs");
    assert!(both.write_glob_matches("a/b.todo"));
    // `project-write = false` asks for nothing, so needs nothing.
    assert!(parse("project-write = false").is_ok());
}
