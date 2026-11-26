//! rBrowser's own sign-in dialog (RB-099-04): a challenge the stored credentials do not answer -
//! another site's, a proxy's, a refused password's - is asked in the shell's bar, never in a page.
//! The core asks with [`AuthPrompt`] (`authPrompt`, again with the same id when more views wait
//! on it) and withdraws it with `authPromptClosed`; the shell answers with [`AuthPromptReply`]
//! (`authPromptReply`): user and password, or nothing for Cancel. The environment a credential is
//! saved for is the core's, never the shell's. No `Debug` output carries the user or the
//! password. Mirror: `ui/src/ipc/authPrompt.ts`.

use std::fmt;

use rb_engine::ViewId;
use serde::{Deserialize, Serialize};

/// One open sign-in question: one per origin, realm and proxy, whichever views wait on it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthPrompt {
    /// The question's id, for the reply.
    pub id: u64,
    /// `scheme://host:port` of the server, `host:port` of a proxy.
    pub origin: String,
    /// The realm the server names; absent when it names none.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub realm: String,
    /// Whether a proxy asks.
    pub proxy: bool,
    /// The views waiting, in order.
    pub views: Vec<ViewId>,
    /// The environment whose address this is, when its credential may be stored: "Save for
    /// environment ..." is offered only then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// Whether the server refused the credential sent before (stored or typed).
    pub refused: bool,
    /// Whether the answer would cross the network unencrypted (plain `http` to another machine).
    #[serde(default)]
    pub insecure: bool,
}

/// The person's answer to the question `id`.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthPromptReply {
    /// The question's id.
    pub id: u64,
    /// The credential; absent for Cancel.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<AuthPromptSignIn>,
}

/// The user and password typed, and whether to store them for the question's environment.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthPromptSignIn {
    /// The user name.
    pub user: String,
    /// The password: a secret.
    pub password: String,
    /// Store it for the environment the question names.
    #[serde(default)]
    pub save: bool,
}

impl fmt::Debug for AuthPromptReply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthPromptReply")
            .field("id", &self.id)
            .field("sign_in", &self.sign_in)
            .finish()
    }
}

/// Neither the user nor the password: an IPC failure writes the message with `{:?}`.
impl fmt::Debug for AuthPromptSignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthPromptSignIn")
            .field("user", &"<redacted>")
            .field("password", &"<redacted>")
            .field("save", &self.save)
            .finish()
    }
}

#[cfg(test)]
#[path = "auth_prompt_tests.rs"]
mod tests;
