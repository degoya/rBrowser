//! The mocks as the panel and MCP see and change them: `mocks` (every file and rule with its
//! state and hits; without workspace trust only the file names, marked blocked), `mock_add`
//! (under trust only, into the plugin's own `mocks/added.json`), `mock_remove` (out of the
//! plugin's own files; a rule of a project file is switched off instead - the project's files
//! are never written) and `mock_enable`.

use rb_plugin_api::{Failure, FailureKind, PluginHost};
use serde_json::{Value, json};

use crate::context::{Context, Settings};
use crate::mocks::{self, Kind, MockFile};
use crate::{bad_args, coded, rules, store};

/// The most rules of the plugin's `added.json`.
pub const MAX_ADDED: usize = 256;

fn rule_entry(file: &MockFile, rule: &mocks::MockRule, context: &Context, off: bool) -> Value {
    let id = file.qualified(rule);
    let mut entry = json!({
        "id": id, "kind": rule.kind.name(), "urlPattern": rule.url_pattern, "regex": rule.regex,
        "method": rule.method, "resourceType": rule.resource_type,
        "enabled": rule.enabled && !off,
        "hits": context.hits.get(&id).copied().unwrap_or(0),
    });
    if let Kind::Mock {
        status, delay_ms, ..
    } = &rule.kind
    {
        entry["status"] = json!(status);
        entry["delayMs"] = json!(delay_ms);
    }
    if let Some((_, reason)) = context
        .rejected
        .iter()
        .find(|(rejected, _)| *rejected == id)
    {
        entry["rejected"] = json!(reason);
    }
    entry
}

/// `mocks`: the mock files and their rules, the session's switches and the block list.
#[must_use]
pub fn overview(host: &mut dyn PluginHost, args: &Value) -> Value {
    let context = Context::read(args);
    let settings = Settings::read(host);
    let state = store::read(host, &context.session);
    let files: Vec<Value> = if context.trusted {
        mocks::load(host, &settings.mocks_dir)
            .iter()
            .map(|file| {
                let rules: Vec<Value> = file
                    .rules
                    .iter()
                    .map(|rule| {
                        let off = state.disabled.contains(&file.qualified(rule));
                        rule_entry(file, rule, &context, off)
                    })
                    .collect();
                json!({ "path": file.path, "owned": file.owned, "blocked": false,
                        "problems": file.problems.iter().take(20).collect::<Vec<_>>(),
                        "rules": rules })
            })
            .collect()
    } else {
        let mut names = mocks::project_files(host, &settings.mocks_dir);
        names.extend(mocks::project_files(host, mocks::OWN_PATH));
        names
            .into_iter()
            .map(|path| json!({ "path": path, "blocked": true, "rules": [] }))
            .collect()
    };
    let block: Vec<Value> = settings
        .block
        .iter()
        .enumerate()
        .map(|(index, pattern)| {
            let id = format!("{}{index}", rules::BLOCK_PREFIX);
            json!({ "id": id, "urlPattern": pattern,
                    "hits": context.hits.get(&id).copied().unwrap_or(0) })
        })
        .collect();
    json!({
        "trusted": context.trusted,
        "mocksEnabled": state.mocks_enabled(&settings),
        "cacheDisabled": state.cache_disabled(&settings),
        "dir": settings.mocks_dir,
        "files": files,
        "block": block,
    })
}

fn added_path() -> String {
    format!("{}/{}", mocks::OWN_PATH, mocks::ADDED_FILE)
}

fn read_own(host: &mut dyn PluginHost, name: &str) -> MockFile {
    let path = format!("{}/{name}", mocks::OWN_PATH);
    let prefix = format!("own:{name}");
    match host.project_read(&path) {
        Ok(bytes) => mocks::parse_file(&bytes, &path, &prefix, true),
        Err(_) => mocks::parse_file(br#"{"rules":[]}"#, &path, &prefix, true),
    }
}

fn write_own(host: &mut dyn PluginHost, file: &MockFile) -> Result<(), Failure> {
    let name = file.path.rsplit('/').next().unwrap_or(mocks::ADDED_FILE);
    host.out_write(
        &format!("{}/{name}", mocks::OWN_DIR),
        mocks::file_json(&file.rules).to_string().as_bytes(),
    )
}

/// `mock_add`: a rule in the mock file form (`id` optional) into `added.json`; one with the same
/// id is replaced.
///
/// # Errors
///
/// `plugin.network-panel.errors.untrusted` without workspace trust, `...errors.bad_rule` naming
/// what does not read, `...errors.too_many` past [`MAX_ADDED`], or the write's failure.
pub fn add(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    if !Context::read(args).trusted {
        return Err(coded(FailureKind::AccessDenied, "errors.untrusted"));
    }
    let mut file = read_own(host, mocks::ADDED_FILE);
    let mut raw = json!({ "match": args["match"], "mock": args["mock"],
                          "block": args["block"], "rewrite": args["rewrite"] });
    raw["id"] = match &args["id"] {
        Value::Null => {
            let next = (1..)
                .map(|n| format!("rule-{n}"))
                .find(|id| !file.rules.iter().any(|rule| rule.id == *id))
                .unwrap_or_default();
            json!(next)
        }
        id => id.clone(),
    };
    let rule = mocks::parse_rule(&raw, file.rules.len()).map_err(|reason| {
        coded(FailureKind::BadData, "errors.bad_rule").with_param("reason", reason)
    })?;
    file.rules.retain(|known| known.id != rule.id);
    if file.rules.len() >= MAX_ADDED {
        return Err(
            coded(FailureKind::Limit, "errors.too_many").with_param("limit", MAX_ADDED.to_string())
        );
    }
    let id = file.qualified(&rule);
    let written = mocks::rule_json(&rule);
    file.rules.push(rule);
    write_own(host, &file)?;
    Ok(json!({ "id": id, "file": added_path(), "rule": written,
               "$post": [{ "target": "panel", "message": { "mocksChanged": true } }] }))
}

fn id_arg(args: &Value) -> Result<(&str, &str), Failure> {
    args["id"]
        .as_str()
        .filter(|id| id.len() <= 200)
        .and_then(|id| id.split_once('#'))
        .filter(|(prefix, local)| !prefix.is_empty() && mocks::valid_id(local))
        .ok_or_else(|| bad_args("id"))
}

/// `mock_remove`: a rule of the plugin's own files is removed; one of a project file is switched
/// off (the project's files are never written).
///
/// # Errors
///
/// `plugin.network-panel.errors.unknown_rule` for an own rule that does not exist, arguments that
/// do not read, or a write's failure.
pub fn remove(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let (prefix, local) = id_arg(args)?;
    let id = format!("{prefix}#{local}");
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    let answer = if let Some(name) = prefix.strip_prefix("own:") {
        if !(name.ends_with(".json")
            && mocks::relative_path(name)
            && mocks::prefix_of(name) == name)
        {
            return Err(bad_args("id"));
        }
        let mut file = read_own(host, name);
        let before = file.rules.len();
        file.rules.retain(|rule| rule.id != local);
        if file.rules.len() == before {
            return Err(coded(FailureKind::NotFound, "errors.unknown_rule").with_param("id", &id));
        }
        write_own(host, &file)?;
        state.disabled.remove(&id);
        json!({ "id": id, "removed": true })
    } else {
        state.disabled.insert(id.clone());
        json!({ "id": id, "removed": false, "disabled": true })
    };
    store::write(host, &state)?;
    let mut answer = answer;
    answer["$post"] = json!([{ "target": "panel", "message": { "mocksChanged": true } }]);
    Ok(answer)
}

/// `mock_enable`: switches one rule on or off (kept across sessions).
///
/// # Errors
///
/// Arguments that do not read, or the state cannot be written.
pub fn enable(host: &mut dyn PluginHost, args: &Value) -> Result<Value, Failure> {
    let (prefix, local) = id_arg(args)?;
    let id = format!("{prefix}#{local}");
    let enabled = args["enabled"]
        .as_bool()
        .ok_or_else(|| bad_args("enabled"))?;
    let context = Context::read(args);
    let mut state = store::read(host, &context.session);
    if enabled {
        state.disabled.remove(&id);
    } else {
        state.disabled.insert(id.clone());
    }
    store::write(host, &state)?;
    Ok(json!({ "id": id, "enabled": enabled,
               "$post": [{ "target": "panel", "message": { "mocksChanged": true } }] }))
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
