use std::path::PathBuf;

use rb_config::{ConfigFile, UserSettings};
use rb_settings::Scope;
use serde_json::{Value, json};

use super::*;
use crate::settings_window::values::{Documents, ProjectDocuments};
use crate::settings_window::write::{Caller, Plan, plan};

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
    let also = drop_dangling(&project, ConfigFile::Project, &mut document);
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
        drop_dangling(&project, ConfigFile::Project, &mut document),
        None
    );
    let project = project_with_unrelated_local();
    let mut document = project_file();
    document["breakpoints"] = without_tablet();
    assert_eq!(
        drop_dangling(&project, ConfigFile::Project, &mut document),
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
        drop_dangling(&project, ConfigFile::Project, &mut document),
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
