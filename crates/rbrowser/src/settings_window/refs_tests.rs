use std::path::PathBuf;

use rb_config::{ConfigFile, UserSettings};
use rb_settings::Scope;
use serde_json::{Value, json};

use super::*;
use crate::settings_window::values::{Documents, ProjectDocuments};
use crate::settings_window::write::{Caller, Plan, plan, validate_project};

fn project_file() -> Value {
    json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "tablet", "width": 768, "height": 1024 },
            { "id": "desktop", "width": 1440, "height": 900 }
        ],
        "layout": {
            "columns": 3, "rows": 1,
            "cells": [
                { "breakpoint": "mobile", "col": 1, "row": 1 },
                { "breakpoint": "tablet", "col": 2, "row": 1 },
                { "breakpoint": "desktop", "col": 3, "row": 1 }
            ]
        }
    })
}

fn local_file() -> Value {
    json!({ "layout": { "hidden": ["tablet"] } })
}

fn project(local: Option<Value>) -> ProjectDocuments {
    ProjectDocuments {
        root: PathBuf::from("/work/site"),
        effective: json!({}),
        project_file: project_file(),
        local_file: local,
    }
}

fn without_tablet() -> Value {
    json!([
        { "id": "mobile", "width": 393, "height": 852 },
        { "id": "desktop", "width": 1440, "height": 900 }
    ])
}

#[test]
fn removing_a_breakpoint_takes_its_cells_and_the_local_hidden_entry() {
    let project = project(Some(local_file()));
    let mut document = project_file();
    document["breakpoints"] = without_tablet();
    let also = drop_dangling(
        &project,
        ConfigFile::Project,
        &mut document,
        project.local_file.clone(),
    );
    assert_eq!(
        document["layout"]["cells"].as_array().map(|cells| cells
            .iter()
            .map(|cell| cell["breakpoint"].clone())
            .collect::<Vec<_>>()),
        Some(vec![json!("mobile"), json!("desktop")])
    );
    assert_eq!(
        also,
        Some((ConfigFile::Local, json!({ "layout": { "hidden": [] } })))
    );
}

#[test]
fn an_untouched_other_file_is_not_written() {
    let project = project(None);
    let mut document = project_file();
    document["breakpoints"] = without_tablet();
    assert_eq!(
        drop_dangling(
            &project,
            ConfigFile::Project,
            &mut document,
            project.local_file.clone()
        ),
        None
    );
    let project = project_with_unrelated_local();
    let mut document = project_file();
    document["breakpoints"] = without_tablet();
    assert_eq!(
        drop_dangling(
            &project,
            ConfigFile::Project,
            &mut document,
            project.local_file.clone()
        ),
        None
    );
}

fn project_with_unrelated_local() -> ProjectDocuments {
    project(Some(json!({ "sync": { "scroll": false } })))
}

#[test]
fn a_write_that_removes_no_breakpoint_changes_no_reference() {
    let project = project(Some(local_file()));
    let mut document = project_file();
    document["layout"]["gap"] = json!(12);
    let before = document.clone();
    assert_eq!(
        drop_dangling(
            &project,
            ConfigFile::Project,
            &mut document,
            project.local_file.clone()
        ),
        None
    );
    assert_eq!(document, before);
}

/// The owner's case (RB-090-61): Delete in Settings > Breakpoints was refused with
/// `/layout/cells/1/breakpoint` and `/layout/hidden/0` - now the write goes through and takes
/// the references along, in both files.
#[test]
fn the_settings_write_of_the_breakpoints_removes_the_references_and_validates() {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    let documents = Documents {
        user: serde_json::to_value(UserSettings::default()).expect("serializes"),
        user_file: None,
        project: Some(project(Some(local_file()))),
    };
    let planned = plan(
        &registry,
        &documents,
        "breakpoints",
        &without_tablet(),
        Scope::Project,
        Caller::Shell,
    );
    let (file, document, also) = match planned {
        Ok(Plan::Project {
            file,
            document,
            also,
        }) => (file, document, also),
        other => panic!("planned: {other:?}"),
    };
    assert_eq!(file, ConfigFile::Project);
    assert_eq!(document["breakpoints"], without_tablet());
    assert_eq!(
        document["layout"]["cells"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        also,
        Some((ConfigFile::Local, json!({ "layout": { "hidden": [] } })))
    );
}

/// RB-094-25: the project's breakpoints in `config.json`, views added at a size in
/// `config.local.json` (`view.add_size` writes there, with the grid). The two lists merge by id.
fn split_files() -> (Value, Value) {
    let project_file = json!({
        "version": 1,
        "breakpoints": [
            { "id": "mobile", "width": 393, "height": 852 },
            { "id": "wide", "label": "1600 px", "width": 1600, "height": 1000 },
            { "id": "desktop", "width": 1920, "height": 1200 }
        ],
        "layout": {
            "columns": 3, "rows": 1,
            "cells": [
                { "breakpoint": "mobile", "col": 1, "row": 1 },
                { "breakpoint": "wide", "col": 2, "row": 1 },
                { "breakpoint": "desktop", "col": 3, "row": 1 }
            ]
        }
    });
    let local_file = json!({
        "breakpoints": [{ "id": "size-1280x800", "width": 1280, "height": 800 }],
        "layout": {
            "columns": 4, "rows": 1,
            "cells": [
                { "breakpoint": "mobile", "col": 1, "row": 1 },
                { "breakpoint": "wide", "col": 2, "row": 1 },
                { "breakpoint": "desktop", "col": 3, "row": 1 },
                { "breakpoint": "size-1280x800", "col": 4, "row": 1 }
            ],
            "hidden": ["wide"]
        }
    });
    (project_file, local_file)
}

/// The config in effect over the two files, serialized as the settings window holds it.
fn effective(project_file: &Value, local_file: Option<&Value>) -> Value {
    let mut merged = project_file.clone();
    if let Some(local) = local_file {
        rb_config::merge(&mut merged, local.clone());
    }
    let config: rb_config::ProjectConfig = serde_json::from_value(merged).expect("a config");
    serde_json::to_value(config).expect("serializes")
}

fn split_documents(project_file: Value, local_file: Value) -> Documents {
    Documents {
        user: serde_json::to_value(UserSettings::default()).expect("serializes"),
        user_file: None,
        project: Some(ProjectDocuments {
            root: PathBuf::from("/work/site"),
            effective: effective(&project_file, Some(&local_file)),
            project_file,
            local_file: Some(local_file),
        }),
    }
}

/// The list in effect without `id`: what the settings window sends after a confirmed Delete.
fn list_without(documents: &Documents, id: &str) -> Value {
    let list = documents
        .project
        .as_ref()
        .and_then(|project| project.effective["breakpoints"].as_array())
        .expect("breakpoints in effect");
    Value::Array(
        list.iter()
            .filter(|breakpoint| breakpoint["id"] != json!(id))
            .cloned()
            .collect(),
    )
}

/// Plans the Delete of `id` in `scope` and returns both project files as they are written.
fn delete(documents: &Documents, id: &str, scope: Scope) -> (Value, Value) {
    let (registry, issues) = crate::control::settings::settings_registry();
    assert!(issues.is_empty(), "{issues:?}");
    let project = documents.project.as_ref().expect("a project");
    let planned = plan(
        &registry,
        documents,
        "breakpoints",
        &list_without(documents, id),
        scope,
        Caller::Shell,
    );
    let (file, document, also) = match planned {
        Ok(Plan::Project {
            file,
            document,
            also,
        }) => (file, document, also),
        other => panic!("planned: {other:?}"),
    };
    let mut project_file = project.project_file.clone();
    let mut local_file = project.local_file.clone().unwrap_or_else(|| json!({}));
    for (target, content) in also.into_iter().chain([(file, document)]) {
        match target {
            ConfigFile::Project => project_file = content,
            ConfigFile::Local => local_file = content,
        }
    }
    (project_file, local_file)
}

fn ids_of(document: &Value) -> Vec<String> {
    document["breakpoints"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(|breakpoint| breakpoint["id"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The owner's case: a breakpoint of `config.json` deleted while `config.local.json` holds
/// breakpoints too. The window writes where the list is stored, the local file; before the fix
/// the merge brought the breakpoint back from `config.json` and nothing changed.
#[test]
fn deleting_a_project_breakpoint_while_the_local_file_has_breakpoints_removes_it() {
    let (project_file, local_file) = split_files();
    let documents = split_documents(project_file, local_file);
    let (project_file, local_file) = delete(&documents, "wide", Scope::Local);
    let after = rb_config::breakpoint_refs::defined_ids(&project_file, Some(&local_file));
    assert!(!after.contains("wide"), "still defined: {after:?}");
    assert_eq!(ids_of(&project_file), vec!["mobile", "desktop"]);
    // The local file keeps only its own breakpoint: no copy of the project's list.
    assert_eq!(ids_of(&local_file), vec!["size-1280x800"]);
    assert_eq!(
        project_file["layout"]["cells"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        local_file["layout"]["cells"].as_array().map(Vec::len),
        Some(3)
    );
    assert_eq!(local_file["layout"]["hidden"], json!([]));
    validate_project(&project_file, Some(&local_file), "").expect("valid after the delete");
}

/// The other way round: a breakpoint of `config.local.json` deleted with the project scope.
#[test]
fn deleting_a_local_breakpoint_in_the_project_scope_removes_it() {
    let (project_file, local_file) = split_files();
    let documents = split_documents(project_file, local_file);
    let (project_file, local_file) = delete(&documents, "size-1280x800", Scope::Project);
    let after = rb_config::breakpoint_refs::defined_ids(&project_file, Some(&local_file));
    assert!(!after.contains("size-1280x800"), "still defined: {after:?}");
    // `config.json`, checked in, takes no breakpoint of this machine along.
    assert_eq!(ids_of(&project_file), vec!["mobile", "wide", "desktop"]);
    assert!(ids_of(&local_file).is_empty());
    assert_eq!(
        local_file["layout"]["cells"].as_array().map(Vec::len),
        Some(3)
    );
}

/// A breakpoint both files define (a local override) leaves both.
#[test]
fn deleting_a_breakpoint_both_files_define_removes_both_entries() {
    let (project_file, mut local_file) = split_files();
    local_file["breakpoints"] = json!([
        { "id": "size-1280x800", "width": 1280, "height": 800 },
        { "id": "desktop", "width": 2000, "height": 1250 }
    ]);
    let documents = split_documents(project_file, local_file);
    let (project_file, local_file) = delete(&documents, "desktop", Scope::Local);
    assert_eq!(ids_of(&project_file), vec!["mobile", "wide"]);
    assert_eq!(ids_of(&local_file), vec!["size-1280x800"]);
    validate_project(&project_file, Some(&local_file), "").expect("valid after the delete");
}
