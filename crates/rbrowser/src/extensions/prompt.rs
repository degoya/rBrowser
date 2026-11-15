//! The question in the window before a Web Store extension is installed on an MCP client's
//! request or updated (RB-080-23): `extensionPrompt` to the shell, the person's
//! `extensionDecision` back - the only way it goes on. The CRX is downloaded and verified
//! before the question, so it names the real name and version; nothing is unpacked before the
//! answer. A prompt that is not answered is forgotten after [`PROMPT_TIMEOUT`] (the asker hears
//! `false`), as is the oldest past [`MAX_OPEN_PROMPTS`] and an earlier prompt for the same
//! extension.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::Duration;

use rb_ipc::{CoreToShell, ExtensionPrompt, ExtensionPromptKind, IpcErrorCode, ShellToCore};

use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;

/// How long a prompt waits for the person.
pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(10 * 60);
/// How many prompts stay answerable; older ones are forgotten.
pub const MAX_OPEN_PROMPTS: usize = 8;

/// What an asker hears: `true` to go on.
pub type Decided = Box<dyn FnOnce(bool) + Send>;

/// What is asked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptRequest {
    /// Install or update.
    pub kind: ExtensionPromptKind,
    /// The extension id.
    pub id: String,
    /// The manifest's name.
    pub name: String,
    /// The version that would be installed.
    pub version: String,
    /// The version installed now.
    pub current_version: Option<String>,
}

/// The open prompts.
#[derive(Default)]
pub struct Prompts {
    next: u32,
    open: BTreeMap<u32, (String, Decided)>,
}

impl Prompts {
    /// Opens a prompt for `request`; `done` hears the answer once.
    pub fn ask(&mut self, request: PromptRequest, done: Decided) -> ExtensionPrompt {
        let earlier: Vec<u32> = self
            .open
            .iter()
            .filter(|(_, (id, _))| *id == request.id)
            .map(|(prompt_id, _)| *prompt_id)
            .collect();
        for prompt_id in earlier {
            self.forget(prompt_id);
        }
        self.next = self.next.wrapping_add(1).max(1);
        let prompt_id = self.next;
        self.open.insert(prompt_id, (request.id.clone(), done));
        while self.open.len() > MAX_OPEN_PROMPTS {
            let Some(oldest) = self.open.keys().next().copied() else {
                break;
            };
            self.forget(oldest);
        }
        ExtensionPrompt {
            prompt_id,
            kind: request.kind,
            id: request.id,
            name: request.name,
            version: request.version,
            current_version: request.current_version,
        }
    }

    /// Takes the open prompt `prompt_id` to answer it; `None` when there is none.
    pub fn decide(&mut self, prompt_id: u32, accepted: bool) -> Option<Decided> {
        let _ = accepted;
        self.open.remove(&prompt_id).map(|(_, done)| done)
    }

    /// Closes `prompt_id` with `false`.
    pub fn forget(&mut self, prompt_id: u32) {
        if let Some((_, done)) = self.open.remove(&prompt_id) {
            done(false);
        }
    }
}

/// One window per process: the prompts live beside the core.
static PROMPTS: Mutex<Prompts> = Mutex::new(Prompts {
    next: 0,
    open: BTreeMap::new(),
});

/// Shows `request` in the window; `done` hears the person's answer, or `false` after
/// [`PROMPT_TIMEOUT`].
pub fn ask(core: &Core, request: PromptRequest, done: Decided) {
    let prompt = lock(&PROMPTS).ask(request, done);
    let prompt_id = prompt.prompt_id;
    tracing::info!(id = %prompt.id, kind = ?prompt.kind, "an extension waits for the person's word");
    core.runtime().spawn(async move {
        tokio::time::sleep(PROMPT_TIMEOUT).await;
        lock(&PROMPTS).forget(prompt_id);
    });
    core.notify(CoreToShell::ExtensionPrompt(prompt));
}

/// The shell's answer to `prompt_id`.
///
/// # Errors
///
/// No open prompt of that id.
pub fn decide(prompt_id: u32, accepted: bool) -> Result<(), Failure> {
    let done = lock(&PROMPTS).decide(prompt_id, accepted).ok_or_else(|| {
        (
            IpcErrorCode::NotFound,
            format!("no open extension prompt {prompt_id}"),
        )
    })?;
    done(accepted);
    Ok(())
}

/// Answers `message` when it is this module's (`extensionDecision`); `None` otherwise.
pub fn ipc(message: &ShellToCore) -> Option<Result<CoreToShell, Failure>> {
    match message {
        ShellToCore::ExtensionDecision {
            prompt_id,
            accepted,
        } => Some(decide(*prompt_id, *accepted).map(|()| CoreToShell::Ack)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
