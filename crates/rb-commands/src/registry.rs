//! The command registry: registration with the binding rules, the effective bindings, lookup
//! by binding and dispatch. The user's bindings live in [`crate::user_bindings`], the leader
//! state machine in [`crate::chord`].

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use crate::validate::{bindings_conflict, check_binding, rejection_key};
use crate::{
    Binding, Chord, CommandDef, CommandError, CommandId, Group, Handler, Key, KeyContext,
    McpCoverage, Platform, ReservedList, Shortcut, builtin_reserved,
};

/// The i18n key of the notice for a plugin default that stays unbound.
pub const NOTICE_BINDING_DROPPED: &str = "shortcuts.notice.binding_dropped";
/// The i18n key of the notice for a leader chord the host gave a plugin command.
pub const NOTICE_LEADER_ASSIGNED: &str = "shortcuts.notice.leader_assigned";
/// The i18n key of the notice for a plugin command reachable only through the palette.
pub const NOTICE_PALETTE_ONLY: &str = "shortcuts.notice.palette_only";
/// The i18n key of the notice for a user binding of a command that does not exist.
pub const NOTICE_UNKNOWN_COMMAND: &str = "shortcuts.notice.unknown_command";
/// The i18n key of the notice for a user binding that was refused.
pub const NOTICE_USER_BINDING_REFUSED: &str = "shortcuts.notice.user_binding_refused";

/// Something the user should hear about bindings: a plugin default left unbound, a user
/// binding that no longer fits. Shown as a notification by the shell (i18n keys, placeholders
/// `{command}`, `{binding}`, `{reason}`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Notice {
    /// The i18n key of the message (`shortcuts.notice.*`).
    pub message_key: String,
    /// The command id.
    pub command: String,
    /// The binding in text form, when one is concerned.
    pub binding: Option<String>,
    /// The i18n key of the reason (`shortcuts.reserved.*`, `shortcuts.rejected.*`).
    pub reason_key: Option<String>,
}

/// One command as the shell, the editor and MCP's `commands_list` see it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSummary {
    /// The command id.
    pub id: CommandId,
    /// The i18n key of the title.
    pub title_key: String,
    /// The leader group.
    pub group: Group,
    /// The effective bindings in text form.
    pub bindings: Vec<String>,
    /// The default bindings in text form ("reset" restores them).
    pub default_bindings: Vec<String>,
    /// Whether the user changed the bindings ("changed only").
    pub changed: bool,
    /// The MCP mapping.
    pub mcp: McpCoverage,
}

pub(crate) struct Registered {
    pub(crate) def: CommandDef,
    pub(crate) handler: Handler,
    /// The defaults in force: the definition's, less what a plugin was refused, plus a leader
    /// chord the host assigned.
    pub(crate) defaults: Vec<Binding>,
}

/// The one registry of commands.
pub struct CommandRegistry {
    pub(crate) commands: BTreeMap<CommandId, Registered>,
    pub(crate) user: BTreeMap<CommandId, Vec<Binding>>,
    pub(crate) reserved: ReservedList,
    pub(crate) platform: Platform,
    pub(crate) notices: Vec<Notice>,
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandRegistry {
    /// The registered command `key` runs with the focus in `context`: the reservation list's
    /// context entry (RB-080-94); `None` without one or when its command is not registered.
    pub fn context_command(&self, key: &Shortcut, context: KeyContext) -> Option<&CommandId> {
        let id = self.reserved.context_command(key, self.platform, context)?;
        self.commands.keys().find(|known| known.as_str() == id)
    }

    /// An empty registry checking against the compiled-in reservation list of this platform
    /// ([`Platform::current`]: macOS on macOS, Windows elsewhere).
    pub fn new() -> Self {
        Self::with_reserved(builtin_reserved().clone(), Platform::current())
    }

    /// An empty registry with its own reservation list and platform.
    pub fn with_reserved(reserved: ReservedList, platform: Platform) -> Self {
        Self {
            commands: BTreeMap::new(),
            user: BTreeMap::new(),
            reserved,
            platform,
            notices: Vec::new(),
        }
    }

    /// Adds a core command and its handler. Refuses a duplicate id, a command without a
    /// leader chord (every command is reachable through the leader), and a default binding
    /// that is malformed, reserved, or held by another command.
    pub fn register(&mut self, def: CommandDef, handler: Handler) -> Result<(), CommandError> {
        if self.commands.contains_key(&def.id) {
            return Err(CommandError::Duplicate(def.id.to_string()));
        }
        if !def
            .default_bindings
            .iter()
            .any(|binding| matches!(binding, Binding::Leader(_)))
        {
            return Err(CommandError::InvalidBinding(format!(
                "{}: no leader chord",
                def.id
            )));
        }
        let mut defaults: Vec<Binding> = Vec::new();
        for binding in &def.default_bindings {
            self.check_for(&def.id, binding)?;
            if defaults.iter().any(|held| bindings_conflict(held, binding)) {
                return Err(CommandError::Conflict {
                    binding: binding.to_string(),
                    other: def.id.to_string(),
                });
            }
            defaults.push(binding.clone());
        }
        self.insert(def, handler, defaults);
        Ok(())
    }

    /// Adds a plugin command (`plugin.<plugin_id>.<command>`). A default that is malformed,
    /// reserved or held already stays unbound, with a notice; a command left without a leader
    /// chord gets a free `Leader X <key>`, or is reachable through the palette only. Returns
    /// the notices (also kept in [`CommandRegistry::notices`]).
    pub fn register_plugin(
        &mut self,
        def: CommandDef,
        handler: Handler,
    ) -> Result<Vec<Notice>, CommandError> {
        if def.id.as_str().split('.').next() != Some("plugin") {
            return Err(CommandError::InvalidId(def.id.to_string()));
        }
        if self.commands.contains_key(&def.id) {
            return Err(CommandError::Duplicate(def.id.to_string()));
        }
        let mut notices = Vec::new();
        let mut defaults: Vec<Binding> = Vec::new();
        for binding in &def.default_bindings {
            let own_conflict = defaults.iter().any(|held| bindings_conflict(held, binding));
            match self.check_for(&def.id, binding) {
                Ok(()) if !own_conflict => defaults.push(binding.clone()),
                result => notices.push(Notice {
                    message_key: NOTICE_BINDING_DROPPED.to_owned(),
                    command: def.id.to_string(),
                    binding: Some(binding.to_string()),
                    reason_key: Some(match result {
                        Err(error) => rejection_key(&error),
                        Ok(()) => crate::validate::REJECTED_CONFLICT.to_owned(),
                    }),
                }),
            }
        }
        if !defaults.iter().any(|b| matches!(b, Binding::Leader(_))) {
            match self.free_plugin_chord(&def.id) {
                Some(chord) => {
                    notices.push(Notice {
                        message_key: NOTICE_LEADER_ASSIGNED.to_owned(),
                        command: def.id.to_string(),
                        binding: Some(chord.to_string()),
                        reason_key: None,
                    });
                    defaults.push(chord);
                }
                None => notices.push(Notice {
                    message_key: NOTICE_PALETTE_ONLY.to_owned(),
                    command: def.id.to_string(),
                    binding: None,
                    reason_key: None,
                }),
            }
        }
        self.insert(def, handler, defaults);
        self.notices.extend(notices.iter().cloned());
        Ok(notices)
    }

    fn insert(&mut self, def: CommandDef, handler: Handler, defaults: Vec<Binding>) {
        self.commands.insert(
            def.id.clone(),
            Registered {
                def,
                handler,
                defaults,
            },
        );
    }

    /// The first free `Leader X <key>` (A-Z, then 0-9).
    fn free_plugin_chord(&self, id: &CommandId) -> Option<Binding> {
        ('A'..='Z')
            .chain('0'..='9')
            .map(|c| {
                Binding::Leader(Chord {
                    keys: vec![Key::Char(Group::Plugins.leader_key()), Key::Char(c)],
                })
            })
            .find(|binding| self.check_for(id, binding).is_ok())
    }

    /// Whether `binding` may be bound to `id`: the shape and reservation rules, and no other
    /// command holds it or a chord it starts or extends. What the editor asks before it
    /// records a binding.
    pub fn check_for(&self, id: &CommandId, binding: &Binding) -> Result<(), CommandError> {
        check_binding(binding, &self.reserved, self.platform)?;
        for (other, _) in self.commands.iter().filter(|(other, _)| *other != id) {
            if self
                .bindings(other)
                .iter()
                .any(|held| bindings_conflict(held, binding))
            {
                return Err(CommandError::Conflict {
                    binding: binding.to_string(),
                    other: other.to_string(),
                });
            }
        }
        Ok(())
    }

    /// The definition of `id`.
    pub fn get(&self, id: &CommandId) -> Option<&CommandDef> {
        self.commands.get(id).map(|registered| &registered.def)
    }

    /// Every command, by id.
    pub fn iter(&self) -> impl Iterator<Item = &CommandDef> {
        self.commands.values().map(|registered| &registered.def)
    }

    /// The commands of one leader group.
    pub fn iter_group(&self, group: Group) -> impl Iterator<Item = &CommandDef> {
        self.iter().filter(move |def| def.group == group)
    }

    /// The defaults in force for `id` (a plugin's less what was refused).
    pub fn default_bindings(&self, id: &CommandId) -> Vec<Binding> {
        self.commands
            .get(id)
            .map(|registered| registered.defaults.clone())
            .unwrap_or_default()
    }

    /// The effective bindings of `id`: the user's when set, else the defaults.
    pub fn bindings(&self, id: &CommandId) -> Vec<Binding> {
        match self.user.get(id) {
            Some(user) if self.commands.contains_key(id) => user.clone(),
            _ => self.default_bindings(id),
        }
    }

    /// The command bound to `binding`, if any.
    pub fn resolve(&self, binding: &Binding) -> Option<&CommandId> {
        self.commands
            .keys()
            .find(|id| self.bindings(id).contains(binding))
    }

    /// Whether some command's leader chord starts with `keys` and is longer.
    pub fn is_chord_prefix(&self, keys: &[Key]) -> bool {
        self.commands.keys().any(|id| {
            self.bindings(id).iter().any(|binding| {
                matches!(binding, Binding::Leader(chord)
                    if chord.keys.len() > keys.len() && chord.keys.starts_with(keys))
            })
        })
    }

    /// The handler of `id`, to run after the registry's lock is released (a handler may take
    /// a while; key presses need the registry meanwhile). A retired id has its successor's
    /// ([`crate::retired`]).
    pub fn handler(&self, id: &CommandId) -> Option<Handler> {
        self.commands
            .get(id)
            .or_else(|| {
                crate::retired::successor(id.as_str())
                    .and_then(|successor| CommandId::new(successor).ok())
                    .and_then(|successor| self.commands.get(&successor))
            })
            .map(|registered| registered.handler.clone())
    }

    /// Runs the command `id` with `args` (a retired id as its successor).
    pub fn dispatch(&self, id: &CommandId, args: Value) -> Result<Value, CommandError> {
        let handler = self
            .handler(id)
            .ok_or_else(|| CommandError::UnknownCommand(id.to_string()))?;
        handler(args)
    }

    /// Every command with its bindings, by id: the `commands` answer to the shell, the
    /// shortcut editor's rows and MCP's `commands_list`.
    pub fn summaries(&self) -> Vec<CommandSummary> {
        self.commands
            .iter()
            .map(|(id, registered)| CommandSummary {
                id: id.clone(),
                title_key: registered.def.title_key.clone(),
                group: registered.def.group,
                bindings: texts(&self.bindings(id)),
                default_bindings: texts(&registered.defaults),
                changed: self.user.contains_key(id),
                mcp: registered.def.mcp.clone(),
            })
            .collect()
    }

    /// The notices collected so far (plugin defaults left unbound or reassigned); those of
    /// the user's bindings are returned by `load_user_bindings` instead.
    pub fn notices(&self) -> &[Notice] {
        &self.notices
    }

    /// Hands the notices over to be shown, and forgets them.
    pub fn take_notices(&mut self) -> Vec<Notice> {
        std::mem::take(&mut self.notices)
    }
}

pub(crate) fn texts(bindings: &[Binding]) -> Vec<String> {
    bindings.iter().map(Binding::to_string).collect()
}

#[cfg(test)]
#[path = "registry_tests.rs"]
pub(crate) mod tests;
