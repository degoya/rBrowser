//! The answer shape of `coverage_get`, `coverage_start` and `coverage_stop`: the files once
//! (`files`, each with an `id`), and per view its line with total, used and unused share per
//! file - compact enough for the panel's 64 KiB messages.

use rb_plugin_api::Finding;
use serde_json::{Value, json};

use crate::model::{FileRun, Run};

/// The most files an answer lists.
pub const MAX_FILES: usize = 300;
/// The most findings an answer lists (all are counted).
pub const MAX_FINDINGS: usize = 100;

/// `run` as an answer, with its findings.
#[must_use]
pub fn summary(run: &Run, findings: &[Finding]) -> Value {
    let mut files: Vec<&FileRun> = Vec::new();
    let mut truncated = false;
    let views: Vec<Value> = run
        .views
        .iter()
        .map(|view| {
            let cells: Vec<Value> = view
                .files
                .iter()
                .filter_map(|file| {
                    let id = match files.iter().position(|known| known.same(file)) {
                        Some(id) => id,
                        None if files.len() < MAX_FILES => {
                            files.push(file);
                            files.len() - 1
                        }
                        None => {
                            truncated = true;
                            return None;
                        }
                    };
                    Some(json!({ "file": id, "total": file.total, "used": file.used,
                                 "unusedPercent": file.unused_percent() }))
                })
                .collect();
            let mut line = json!({
                "view": view.view, "label": view.label, "breakpoint": view.breakpoint,
                "width": view.width, "engine": view.engine, "url": view.url,
                "status": view.status, "files": cells,
            });
            if let Some(error) = &view.error {
                line["error"] = json!(error);
            }
            line
        })
        .collect();
    let files: Vec<Value> = files
        .iter()
        .enumerate()
        .map(|(id, file)| {
            json!({ "id": id, "url": file.url, "kind": file.kind, "inline": file.inline })
        })
        .collect();
    let listed: Vec<Value> = findings
        .iter()
        .take(MAX_FINDINGS)
        .map(|finding| {
            json!({ "rule": finding.rule, "severity": finding.severity, "message": finding.message,
                    "params": finding.params.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<serde_json::Map<_, _>>(),
                    "view": finding.view, "path": finding.path })
        })
        .collect();
    json!({
        "mode": run.mode, "running": run.running, "started": run.started,
        "finished": run.finished, "files": files, "views": views, "truncated": truncated,
        "findingCount": findings.len(), "findings": listed,
    })
}

#[cfg(test)]
#[path = "answer_tests.rs"]
mod tests;
