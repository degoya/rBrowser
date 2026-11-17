//! The local-only rule and the trust gate.
//!
//! [`crate::LOCAL_ONLY_KEYS`] take effect only from `config.local.json`: in `config.json` such a
//! key is removed before the merge, so a shared config can never switch off TLS checks or route
//! the team through a proxy.
//!
//! [`crate::TRUST_GATED_KEYS`] (TLS checks, proxy, host resolver rules) are removed from both
//! project files, `config.local.json` included: a cloned repository can commit that file too,
//! and these keys would silently redirect or intercept every page of the grid. Only a trusted
//! workspace keeps them (`crate::trust::load_project_trusted`, RB-010-09).
//!
//! A removed value that would change something (not `null`, not `false`) gives one warning per
//! key; spelling out the harmless default stays silent, so the plan's example loads without
//! noise.

use serde_json::Value;

use crate::{ConfigIssue, LOCAL_ONLY_KEYS, TRUST_GATED_KEYS};

/// The i18n key of the local-only warning.
pub const WARNING_KEY: &str = "config.warning.local_only_ignored";
/// The i18n key of the trust-gate warning.
pub const TRUST_WARNING_KEY: &str = "config.warning.trust_required";

/// Removes every local-only key from `project` (the parsed `config.json`) and returns one
/// warning for each removed key whose value was not `null` or `false`.
pub fn strip_local_only(project: &mut Value) -> Vec<ConfigIssue> {
    strip(project, LOCAL_ONLY_KEYS, |key, pointer| ConfigIssue {
        path: pointer.to_owned(),
        key: WARNING_KEY.to_owned(),
        message: format!("`{key}` is honoured only in config.local.json; ignored in config.json"),
    })
}

/// Removes every trust-gated key from `value`, the parsed project file `file`
/// (`config.json` or `config.local.json`), with one warning naming key and file for each
/// removed key whose value was not `null` or `false`.
pub fn strip_trust_gated(value: &mut Value, file: &str) -> Vec<ConfigIssue> {
    strip(value, TRUST_GATED_KEYS, |key, pointer| ConfigIssue {
        path: pointer.to_owned(),
        key: TRUST_WARNING_KEY.to_owned(),
        message: format!(
            "`{key}` in {file} is ignored: a project file may not set it until the workspace \
             is trusted"
        ),
    })
}

fn strip(
    value: &mut Value,
    keys: &[&str],
    warning: impl Fn(&str, &str) -> ConfigIssue,
) -> Vec<ConfigIssue> {
    let mut warnings = Vec::new();
    for key in keys {
        let Some((parent, leaf)) = key.rsplit_once('.') else {
            continue;
        };
        let pointer = format!("/{}", parent.replace('.', "/"));
        let Some(object) = value.pointer_mut(&pointer).and_then(Value::as_object_mut) else {
            continue;
        };
        let Some(removed) = object.remove(leaf) else {
            continue;
        };
        if !matches!(removed, Value::Null | Value::Bool(false)) {
            warnings.push(warning(key, &format!("{pointer}/{leaf}")));
        }
    }
    warnings
}

#[cfg(test)]
#[path = "local_only_tests.rs"]
mod tests;
