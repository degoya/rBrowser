use rb_commands::lint::lint_defaults;
use rb_commands::{Platform, builtin_reserved};
use rb_config::PanelDock;
use serde_json::json;

use super::*;
use crate::panel_dock::tests::{docks, setup};

#[test]
fn every_command_has_a_free_key_after_leader_p_and_the_tool() {
    let defs = definitions().expect("valid ids");
    let chords: Vec<String> = defs
        .iter()
        .map(|def| def.default_bindings[0].to_string())
        .collect();
    assert_eq!(
        chords,
        [
            "Ctrl+Shift+Space P Left",
            "Ctrl+Shift+Space P Right",
            "Ctrl+Shift+Space P Down",
            "Ctrl+Shift+Space P Up",
            "Ctrl+Shift+Space P X",
            "Ctrl+Shift+Space P Z",
        ]
    );
    for def in &defs {
        assert_eq!(def.group, Group::Panels);
        assert_eq!(def.mcp, McpCoverage::Covered(TOOL.to_owned()));
        assert_eq!(def.title_key, format!("commands.{}", def.id.as_str()));
    }
    // Beside every other command of the Panels group.
    let mut all = rb_commands::builtin::definitions();
    all.extend(crate::status::defs::definitions().expect("status"));
    all.extend(crate::bookmarks::definitions().expect("bookmarks"));
    all.extend(crate::toolbar_rows::definitions().expect("toolbar"));
    all.extend(crate::sitemap::defs::definitions().expect("sitemap"));
    all.extend(rb_plugin_host::core_command_defs().expect("plugins.*"));
    all.extend(defs);
    let issues: Vec<String> = lint_defaults(&all, builtin_reserved(), Platform::Windows)
        .iter()
        .map(ToString::to_string)
        .collect();
    assert!(issues.is_empty(), "shortcut lint:\n{}", issues.join("\n"));
}

#[tokio::test]
async fn the_registry_runs_them_with_their_arguments() {
    let setup = setup();
    assert!(setup.core.has_command(DOCK_BOTTOM));
    let answer = setup
        .core
        .run_command(DOCK_BOTTOM, json!({ "size": 300 }))
        .expect("docks");
    assert_eq!(
        answer,
        json!({ "position": "bottom", "size": 300, "collapsed": false })
    );
    let answer = setup.core.run_command(TOGGLE, Value::Null).expect("folds");
    assert_eq!(answer["collapsed"], true);
    assert!(
        docks(&setup.ui)
            .last()
            .cloned()
            .flatten()
            .expect("docked")
            .collapsed
    );
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Bottom);
    // Close folds to the rail (`panels.closeTo`), `to: "hidden"` hides (RB-080-93).
    let answer = setup.core.run_command(CLOSE, Value::Null).expect("closes");
    assert_eq!(
        answer,
        json!({ "position": "bottom", "size": 300, "collapsed": true })
    );
    let answer = setup
        .core
        .run_command(CLOSE, json!({ "to": "hidden" }))
        .expect("hides");
    assert_eq!(answer, json!({ "position": "closed", "state": "hidden" }));
    assert_eq!(docks(&setup.ui).last(), Some(&None));
    let answer = setup
        .core
        .run_command(UNDOCK, Value::Null)
        .expect("undocks");
    assert_eq!(answer, json!({ "position": "floating" }));
    assert_eq!(setup.core.user_settings().panels.dock, PanelDock::Floating);
    assert!(matches!(
        setup.core.run_command(DOCK_LEFT, json!({ "size": 1 })),
        Err(CommandError::InvalidArgs(_))
    ));
}

#[tokio::test]
async fn an_unknown_id_is_not_run() {
    let setup = setup();
    assert!(matches!(
        run(&setup.core, "panels.dock.top", &Value::Null),
        Err(CommandError::UnknownCommand(_))
    ));
}
