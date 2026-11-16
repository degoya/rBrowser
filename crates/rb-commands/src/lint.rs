//! The lint of default bindings, with the coverage check: every command has a default binding
//! and a leader chord among them; no default is malformed, reserved or held twice; a two-key
//! leader chord starts with its command's group key. Run over the core definitions and the
//! bundled plugins' manifests by `tests/default_bindings.rs` (part of `scripts/check.sh`), and
//! enforced at registration by [`crate::CommandRegistry::register`].

use std::fmt;

use crate::validate::{REJECTED_LEADER, bindings_conflict, check_binding};
use crate::{Binding, CommandDef, CommandError, Key, Platform, ReservedList};

/// What is wrong with a default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LintKind {
    /// The command has no default binding (the coverage check).
    Uncovered,
    /// No default is a leader chord, so the leader does not reach the command.
    NoLeaderChord,
    /// The id is defined twice.
    DuplicateId,
    /// The binding is on the reservation list; the English reason.
    Reserved(String),
    /// The binding is malformed (see [`crate::validate::check_binding`]).
    Invalid,
    /// Another command (or a group, by its title key) holds the binding or a chord it starts
    /// or extends.
    Conflict(String),
    /// A two-key chord that does not start with the command's group key.
    ChordOutsideGroup(char),
    /// A context entry of the reservation list ([`crate::ContextException`]) that is no exact
    /// combination or names no command; what is wrong.
    Context(String),
}

/// One finding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LintIssue {
    /// The command id.
    pub command: String,
    /// The binding concerned, in text form.
    pub binding: Option<String>,
    /// What is wrong.
    pub kind: LintKind,
}

impl fmt::Display for LintIssue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.command)?;
        if let Some(binding) = &self.binding {
            write!(f, " `{binding}`")?;
        }
        match &self.kind {
            LintKind::Uncovered => write!(f, ": no default shortcut"),
            LintKind::NoLeaderChord => write!(f, ": no leader chord among the defaults"),
            LintKind::DuplicateId => write!(f, ": defined twice"),
            LintKind::Reserved(reason) => write!(f, ": reserved - {reason}"),
            LintKind::Invalid => write!(f, ": not a valid binding"),
            LintKind::Conflict(other) => write!(f, ": collides with {other}"),
            LintKind::ChordOutsideGroup(key) => {
                write!(f, ": a two-key chord starts with the group key `{key}`")
            }
            LintKind::Context(why) => write!(f, ": reservation list context entry - {why}"),
        }
    }
}

/// Lints the default bindings of `defs` against `reserved` on `platform`.
pub fn lint_defaults(
    defs: &[CommandDef],
    reserved: &ReservedList,
    platform: Platform,
) -> Vec<LintIssue> {
    let mut issues = Vec::new();
    let issue = |def: &CommandDef, binding: Option<&Binding>, kind| LintIssue {
        command: def.id.to_string(),
        binding: binding.map(Binding::to_string),
        kind,
    };
    for (index, def) in defs.iter().enumerate() {
        if defs[..index].iter().any(|earlier| earlier.id == def.id) {
            issues.push(issue(def, None, LintKind::DuplicateId));
        }
        if def.default_bindings.is_empty() {
            issues.push(issue(def, None, LintKind::Uncovered));
        } else if !def
            .default_bindings
            .iter()
            .any(|binding| matches!(binding, Binding::Leader(_)))
        {
            issues.push(issue(def, None, LintKind::NoLeaderChord));
        }
        for binding in &def.default_bindings {
            if let Err(error) = check_binding(binding, reserved, platform) {
                issues.push(issue(def, Some(binding), rejection(&error, reserved)));
            }
            let group_key = def.group.leader_key();
            if let Binding::Leader(chord) = binding
                && let [first, _] = chord.keys.as_slice()
                && *first != Key::Char(group_key)
            {
                issues.push(issue(
                    def,
                    Some(binding),
                    LintKind::ChordOutsideGroup(group_key),
                ));
            }
        }
    }
    // Collisions: every pair of defaults, within a command and across commands.
    let all: Vec<(&CommandDef, &Binding)> = defs
        .iter()
        .flat_map(|def| def.default_bindings.iter().map(move |b| (def, b)))
        .collect();
    for (index, (def, binding)) in all.iter().enumerate() {
        if let Some((other, _)) = all[..index]
            .iter()
            .find(|(_, earlier)| bindings_conflict(earlier, binding))
        {
            issues.push(issue(
                def,
                Some(binding),
                LintKind::Conflict(other.id.to_string()),
            ));
        }
    }
    issues
}

/// The context entries of `reserved` against `defs` (RB-080-94): each is one exact combination,
/// without a placeholder, and runs a command `defs` define. Run with every core definition, the
/// app's included (`crates/rbrowser/src/layout/canvas_level_tests.rs`).
pub fn lint_contexts(defs: &[CommandDef], reserved: &ReservedList) -> Vec<LintIssue> {
    reserved
        .entries
        .iter()
        .filter_map(|entry| entry.except.as_ref().map(|except| (entry, except)))
        .filter_map(|(entry, except)| {
            let why =
                if entry.combo.contains('<') || entry.combo.parse::<crate::Shortcut>().is_err() {
                    "a context entry needs one exact combination".to_owned()
                } else if !defs.iter().any(|def| def.id.as_str() == except.command) {
                    format!("no command {} to run", except.command)
                } else {
                    return None;
                };
            Some(LintIssue {
                command: except.command.clone(),
                binding: Some(entry.combo.clone()),
                kind: LintKind::Context(why),
            })
        })
        .collect()
}

fn rejection(error: &CommandError, reserved: &ReservedList) -> LintKind {
    match error {
        CommandError::Reserved(key) if key == REJECTED_LEADER => {
            LintKind::Reserved("the leader itself".to_owned())
        }
        CommandError::Reserved(key) => LintKind::Reserved(
            reserved
                .entries
                .iter()
                .find(|entry| &entry.reason_key == key)
                .map_or_else(|| key.clone(), |entry| entry.reason.clone()),
        ),
        CommandError::Conflict { other, .. } => LintKind::Conflict(other.clone()),
        _ => LintKind::Invalid,
    }
}

#[cfg(test)]
#[path = "lint_tests.rs"]
mod tests;
