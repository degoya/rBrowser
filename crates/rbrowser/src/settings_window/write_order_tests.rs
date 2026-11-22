use std::path::PathBuf;

use rb_config::UserSettings;
use rb_settings::Scope;
use serde_json::json;

use super::*;
use crate::settings_window::values::Documents;
use crate::settings_window::write::{Caller, Plan, plan, validate_project};

/// The config in effect over the two files, serialized as the settings window holds it.
fn effective(project_file: &Value, local_file: Option<&Value>) -> Value {
    let mut merged = project_file.clone();
    if let Some(local) = local_file {
        rb_config::merge(&mut merged, local.clone());
    }
    let config: rb_config::ProjectConfig = serde_json::from_value(merged).expect("a config");
    serde_json::to_value(config).expect("serializes")
}

fn project(project_file: Value, local_file: Value) -> ProjectDocuments {
    ProjectDocuments {
        root: PathBuf::from("/work/site"),
        effective: effective(&project_file, Some(&local_file)),
        project_file,
        local_file: Some(local_file),
    }
}

/// Writes `writes` one by one over the stored files and checks the files on disk after each.
fn every_state_on_disk_is_valid(project: &ProjectDocuments, writes: Vec<(ConfigFile, Value)>) {
    let mut project_file = project.project_file.clone();
    let mut local_file = project.local_file.clone();
    for (target, content) in writes {
        match target {
            ConfigFile::Project => project_file = content,
            ConfigFile::Local => local_file = Some(content),
        }
        let checked = validate_project(&project_file, local_file.as_ref(), "");
        assert!(checked.is_ok(), "after {target:?}: {checked:?}");
    }
}

/// RB-094-25: a breakpoint only `config.local.json` defines, named by a cell of `config.json`.
/// Its Delete takes the definition out of the local file and the cell out of the project file:
/// the project file goes first, so the files on disk stay valid between the two writes.
#[test]
fn a_file_that_loses_a_definition_the_other_names_is_written_last() {
    let project = project(
        json!({
            "version": 1,
            "breakpoints": [{ "id": "mobile", "width": 393, "height": 852 }],
            "layout": {
                "columns": 2, "rows": 1,
                "cells": [
                    { "breakpoint": "mobile", "col": 1, "row": 1 },
                    { "breakpoint": "extra", "col": 2, "row": 1 }
                ]
            }
        }),
        json!({ "breakpoints": [{ "id": "extra", "width": 1280, "height": 800 }] }),
    );
    let list: Vec<Value> = project.effective["breakpoints"]
        .as_array()
        .expect("breakpoints in effect")
        .iter()
        .filter(|breakpoint| breakpoint["id"] != json!("extra"))
        .cloned()
        .collect();
    let documents = Documents {
        user: serde_json::to_value(UserSettings::default()).expect("serializes"),
        user_file: None,
        project: Some(project.clone()),
    };
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    let Ok(Plan::Project {
        file,
        document,
        also,
    }) = plan(
        &registry,
        &documents,
        "breakpoints",
        &Value::Array(list),
        Scope::Project,
        Caller::Shell,
    )
    else {
        panic!("planned");
    };
    assert!(also.is_some(), "the local file loses the definition");
    let writes = writes(&project, "breakpoints", file, document, also, &[]).expect("checked");
    let order: Vec<ConfigFile> = writes.iter().map(|(target, _)| *target).collect();
    assert_eq!(order, [ConfigFile::Project, ConfigFile::Local]);
    every_state_on_disk_is_valid(&project, writes);
}

/// RB-090-61: a breakpoint `config.json` defines, hidden in `config.local.json`: the local file
/// loses only the reference and still goes first.
#[test]
fn a_file_that_loses_only_references_is_written_first() {
    let project = project(
        json!({
            "version": 1,
            "breakpoints": [
                { "id": "mobile", "width": 393, "height": 852 },
                { "id": "tablet", "width": 768, "height": 1024 }
            ],
            "layout": {
                "columns": 2, "rows": 1,
                "cells": [
                    { "breakpoint": "mobile", "col": 1, "row": 1 },
                    { "breakpoint": "tablet", "col": 2, "row": 1 }
                ]
            }
        }),
        json!({ "layout": { "hidden": ["tablet"] } }),
    );
    let document = json!({
        "version": 1,
        "breakpoints": [{ "id": "mobile", "width": 393, "height": 852 }],
        "layout": {
            "columns": 2, "rows": 1,
            "cells": [{ "breakpoint": "mobile", "col": 1, "row": 1 }]
        }
    });
    let also = Some((ConfigFile::Local, json!({ "layout": { "hidden": [] } })));
    let writes = writes(
        &project,
        "breakpoints",
        ConfigFile::Project,
        document,
        also,
        &[],
    )
    .expect("checked");
    let order: Vec<ConfigFile> = writes.iter().map(|(target, _)| *target).collect();
    assert_eq!(order, [ConfigFile::Local, ConfigFile::Project]);
    every_state_on_disk_is_valid(&project, writes);
}

fn renamed() -> Vec<(String, String)> {
    vec![("staging".to_owned(), "stage".to_owned())]
}

/// RB-094-24: the file that defines the renamed environment goes first; the other one names the
/// new id only once it exists on disk.
#[test]
fn a_rename_writes_the_file_of_the_environments_first() {
    let project = project(
        json!({
            "version": 1,
            "environments": [{ "id": "staging", "url": "https://s.example" }]
        }),
        json!({ "basicAuth": { "environments": ["staging"] } }),
    );
    let document = json!({
        "version": 1,
        "environments": [{ "id": "stage", "url": "https://s.example" }]
    });
    let also = Some((
        ConfigFile::Local,
        json!({ "basicAuth": { "environments": ["stage"] } }),
    ));
    let writes = writes(
        &project,
        "environments",
        ConfigFile::Project,
        document,
        also,
        &renamed(),
    )
    .expect("checked");
    let order: Vec<ConfigFile> = writes.iter().map(|(target, _)| *target).collect();
    assert_eq!(order, [ConfigFile::Project, ConfigFile::Local]);
}

/// What the rename carried along is checked like the plan: a config that would not load is
/// refused before anything is written.
#[test]
fn what_a_rename_carried_is_checked_before_it_is_written() {
    let project = project(
        json!({
            "version": 1,
            "environments": [{ "id": "staging", "url": "https://s.example" }]
        }),
        json!({}),
    );
    let document = json!({
        "version": 1,
        "environments": [
            { "id": "stage", "url": "https://s.example" },
            { "id": "stage", "url": "https://t.example" }
        ]
    });
    let refused = writes(
        &project,
        "environments",
        ConfigFile::Project,
        document,
        None,
        &renamed(),
    );
    assert!(refused.is_err(), "{refused:?}");
}
