//! The shortcut lint and coverage check of `check.sh`: the default bindings of the core
//! definitions and of every bundled plugin (`plugins/*/manifest.toml`, `[[commands]]` with
//! `shortcut` or `shortcuts`, `group`) against the reservation list and each other.

use std::fs;
use std::path::{Path, PathBuf};

use rb_commands::lint::{LintKind, lint_defaults};
use rb_commands::validate::group_of_key;
use rb_commands::{
    Binding, CommandDef, CommandId, Group, McpCoverage, Platform, builtin, builtin_reserved,
};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/rb-commands sits two levels below the root")
        .to_path_buf()
}

/// The command definitions of one manifest; problems that are no binding issue (an id that
/// does not form a command id, a shortcut that does not parse) come back as messages.
fn manifest_defs(path: &Path, text: &str) -> (Vec<CommandDef>, Vec<String>) {
    let mut defs = Vec::new();
    let mut problems = Vec::new();
    let manifest: toml::Table = match text.parse() {
        Ok(table) => table,
        Err(error) => return (defs, vec![format!("{}: {error}", path.display())]),
    };
    let plugin_id = manifest
        .get("id")
        .or_else(|| manifest.get("plugin").and_then(|plugin| plugin.get("id")))
        .and_then(toml::Value::as_str)
        .unwrap_or("unknown")
        .replace('-', "_");
    let commands = manifest
        .get("commands")
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for command in commands {
        let local = command
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let local = local
            .strip_prefix(&format!("{plugin_id}."))
            .unwrap_or(local)
            .replace('-', "_");
        let text_id = format!("plugin.{plugin_id}.{local}");
        let Ok(id) = CommandId::new(&text_id) else {
            problems.push(format!("{}: invalid command id {text_id}", path.display()));
            continue;
        };
        let texts: Vec<String> = match (command.get("shortcut"), command.get("shortcuts")) {
            (Some(one), _) => one.as_str().map(String::from).into_iter().collect(),
            (None, Some(many)) => many
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            (None, None) => Vec::new(),
        };
        let mut bindings = Vec::new();
        for text in texts {
            match text.parse::<Binding>() {
                Ok(binding) => bindings.push(binding),
                Err(error) => problems.push(format!("{} {text_id}: {error}", path.display())),
            }
        }
        // A plugin command belongs to the group its manifest names, else the one its chord
        // opens; without either, to Plugins (as the host registers it).
        let named = command
            .get("group")
            .and_then(toml::Value::as_str)
            .and_then(|group| {
                serde_json::from_value::<Group>(serde_json::Value::String(group.to_owned())).ok()
            });
        let group = named
            .or_else(|| {
                bindings.iter().find_map(|binding| match binding {
                    Binding::Leader(chord) if chord.keys.len() == 2 => group_of_key(chord.keys[0]),
                    _ => None,
                })
            })
            .unwrap_or(Group::Plugins);
        defs.push(CommandDef {
            title_key: format!("commands.{text_id}"),
            id,
            group,
            default_bindings: bindings,
            mcp: McpCoverage::Omitted("lint".into()),
        });
    }
    (defs, problems)
}

fn bundled_plugin_defs() -> (Vec<CommandDef>, Vec<String>) {
    let mut defs = Vec::new();
    let mut problems = Vec::new();
    let Ok(entries) = fs::read_dir(workspace_root().join("plugins")) else {
        return (defs, problems);
    };
    let mut manifests: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("manifest.toml"))
        .filter(|path| path.is_file())
        .collect();
    manifests.sort();
    for path in manifests {
        match fs::read_to_string(&path) {
            Ok(text) => {
                let (more, more_problems) = manifest_defs(&path, &text);
                defs.extend(more);
                problems.extend(more_problems);
            }
            Err(error) => problems.push(format!("{}: {error}", path.display())),
        }
    }
    (defs, problems)
}

/// Every core command of the app - the ones the `rbrowser` crate registers beside
/// [`builtin`] included - from the generated `schema/commands.v1.json` (a test of the app holds
/// it current), so a core default that takes a bundled plugin's chord fails here.
fn app_defs() -> (Vec<CommandDef>, Vec<String>) {
    let path = workspace_root().join("schema/commands.v1.json");
    let file: serde_json::Value = match fs::read_to_string(&path)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
    {
        Ok(file) => file,
        Err(error) => return (Vec::new(), vec![format!("{}: {error}", path.display())]),
    };
    let mut defs = builtin::definitions();
    let mut problems = Vec::new();
    let commands = file["commands"].as_array().cloned().unwrap_or_default();
    for command in commands {
        let text_id = command["id"].as_str().unwrap_or("");
        if defs.iter().any(|def| def.id.as_str() == text_id) {
            continue;
        }
        let Ok(id) = CommandId::new(text_id) else {
            problems.push(format!("{}: invalid command id {text_id}", path.display()));
            continue;
        };
        let Ok(group) = serde_json::from_value::<Group>(command["group"].clone()) else {
            problems.push(format!("{}: {text_id} has no group", path.display()));
            continue;
        };
        let mut bindings = Vec::new();
        for text in command["defaultBindings"].as_array().into_iter().flatten() {
            match text.as_str().unwrap_or("").parse::<Binding>() {
                Ok(binding) => bindings.push(binding),
                Err(error) => problems.push(format!("{} {text_id}: {error}", path.display())),
            }
        }
        defs.push(CommandDef {
            title_key: format!("commands.{text_id}"),
            id,
            group,
            default_bindings: bindings,
            mcp: McpCoverage::Omitted("lint".into()),
        });
    }
    (defs, problems)
}

#[test]
fn the_lint_reads_every_core_command_of_the_app() {
    let (defs, problems) = app_defs();
    assert!(problems.is_empty(), "{problems:?}");
    // Registered by the rbrowser crate, not by `builtin`.
    assert!(defs.iter().any(|def| def.id.as_str() == "status.history"));
    assert!(defs.len() > builtin::definitions().len());
}

#[test]
fn every_default_binding_passes_the_reserved_list_and_collides_with_none() {
    let (plugins, problems) = bundled_plugin_defs();
    let (mut defs, app_problems) = app_defs();
    defs.extend(plugins);
    let problems: Vec<String> = problems.into_iter().chain(app_problems).collect();
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .chain(problems)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

/// The same defaults against the macOS reservations (RB-090-15): the leader and the direct
/// shortcuts stay free of the Command key, Spotlight, the input sources and Mission Control.
#[test]
fn every_default_binding_passes_the_macos_list() {
    let (plugins, problems) = bundled_plugin_defs();
    let (mut defs, app_problems) = app_defs();
    defs.extend(plugins);
    let problems: Vec<String> = problems.into_iter().chain(app_problems).collect();
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Macos)
        .iter()
        .map(ToString::to_string)
        .chain(problems)
        .collect();
    assert!(
        issues.is_empty(),
        "shortcut lint (macOS):\n{}",
        issues.join("\n")
    );
}

/// The same defaults against the Linux reservations (RB-097-05): the leader and the direct
/// shortcuts stay free of the Super key, the window manager's Alt and Ctrl+Alt keys and the
/// workspaces.
#[test]
fn every_default_binding_passes_the_linux_list() {
    let (plugins, problems) = bundled_plugin_defs();
    let (mut defs, app_problems) = app_defs();
    defs.extend(plugins);
    let problems: Vec<String> = problems.into_iter().chain(app_problems).collect();
    let issues: Vec<String> = lint_defaults(&defs, builtin_reserved(), Platform::Linux)
        .iter()
        .map(ToString::to_string)
        .chain(problems)
        .collect();
    assert!(
        issues.is_empty(),
        "shortcut lint (Linux):\n{}",
        issues.join("\n")
    );
}

#[test]
fn the_lint_reads_every_bundled_manifest() {
    let (defs, problems) = bundled_plugin_defs();
    assert!(problems.is_empty(), "{problems:?}");
    let hello = defs
        .iter()
        .find(|def| def.id.as_str() == "plugin.hello_panel.say_hello")
        .expect("plugins/hello-panel/manifest.toml is read");
    assert_eq!(hello.group, Group::Plugins);
    assert_eq!(hello.default_bindings.len(), 1);
}

#[test]
fn a_manifests_group_names_the_commands_group() {
    let manifest = r#"
        id = "seo"
        [[commands]]
        id = "report"
        group = "checks"
        shortcut = "Ctrl+Shift+Space K R"
    "#;
    let (defs, problems) = manifest_defs(Path::new("plugins/seo/manifest.toml"), manifest);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(defs[0].id.as_str(), "plugin.seo.report");
    assert_eq!(defs[0].group, Group::Checks);
}

#[test]
fn the_lint_reads_a_manifest_and_fails_on_a_reserved_default() {
    let manifest = r#"
        id = "seo"
        [[commands]]
        id = "seo.check"
        title = "commands.seo.check"
        shortcut = "Leader K S"
        [[commands]]
        id = "seo.close"
        shortcuts = ["Ctrl+T", "Leader X Q"]
        [[commands]]
        id = "seo.bare"
    "#;
    let (defs, problems) = manifest_defs(Path::new("plugins/seo/manifest.toml"), manifest);
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(defs[0].id.as_str(), "plugin.seo.check");
    assert_eq!(defs[0].group, Group::Checks);
    let kinds: Vec<(String, LintKind)> =
        lint_defaults(&defs, builtin_reserved(), Platform::Windows)
            .into_iter()
            .map(|issue| (issue.command, issue.kind))
            .collect();
    assert_eq!(kinds.len(), 2, "{kinds:#?}");
    assert_eq!(kinds[0].0, "plugin.seo.close");
    assert!(
        matches!(&kinds[0].1, LintKind::Reserved(reason) if reason.starts_with("Browser: new tab"))
    );
    assert_eq!(
        kinds[1],
        ("plugin.seo.bare".to_owned(), LintKind::Uncovered)
    );
}

/// Every default binding of the app and the bundled plugins, typed into the leader machine over
/// a registry holding every command, runs its own command: a chord that only shows a group, is
/// swallowed or passes through would be a key that "does nothing" (RB-080-18).
#[test]
fn every_default_chord_walks_the_machine_to_its_command() {
    use rb_commands::{
        ChordMachine, CommandRegistry, Focus, KeyOutcome, LEADER, Modifiers, Shortcut,
    };
    use std::sync::Arc;
    use std::time::Instant;

    let (mut defs, problems) = app_defs();
    let (plugins, plugin_problems) = bundled_plugin_defs();
    assert!(
        problems.is_empty() && plugin_problems.is_empty(),
        "{problems:?} {plugin_problems:?}"
    );
    defs.extend(plugins);
    let mut registry = CommandRegistry::new();
    for def in &defs {
        let handler: rb_commands::Handler = Arc::new(|_| Ok(serde_json::Value::Null));
        let registered = if def.id.as_str().starts_with("plugin.") {
            registry.register_plugin(def.clone(), handler).map(|_| ())
        } else {
            registry.register(def.clone(), handler)
        };
        registered.unwrap_or_else(|error| panic!("{}: {error}", def.id));
    }
    let now = Instant::now();
    let mut walked = 0;
    let mut wrong = Vec::new();
    for def in &defs {
        for binding in &def.default_bindings {
            let mut machine = ChordMachine::default();
            let outcome = match binding {
                Binding::Direct(shortcut) => machine.on_key(&registry, Focus::View, *shortcut, now),
                Binding::Leader(chord) => {
                    let mut outcome = machine.on_key(&registry, Focus::View, LEADER, now);
                    for key in &chord.keys {
                        let shortcut = Shortcut {
                            modifiers: Modifiers::NONE,
                            key: *key,
                        };
                        outcome = machine.on_key(&registry, Focus::View, shortcut, now);
                    }
                    outcome
                }
            };
            walked += 1;
            if outcome != KeyOutcome::Run(def.id.clone()) {
                wrong.push(format!("{} {binding}: {outcome:?}", def.id));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "chords that do not reach their command:\n{}",
        wrong.join("\n")
    );
    assert!(walked > 100, "{walked} bindings walked");
    // The two chords of the owner's report (RB-080-18).
    for id in ["debug.vitals", "plugin.seo.check"] {
        assert!(
            defs.iter().any(|def| def.id.as_str() == id),
            "{id} is registered"
        );
    }
}
