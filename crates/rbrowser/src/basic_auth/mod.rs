//! Basic auth per environment (RB-094-08): a staging behind HTTP Basic auth loads in every view
//! and every session without a prompt, once a person stored its user and password.
//!
//! | Part | Where |
//! | --- | --- |
//! | Which environments are behind Basic auth (`basicAuth.environments`, `x-protected`) | `rb_config::BasicAuthConfig` |
//! | The origin a credential belongs to: https, http on this machine only, no user info | [`origin`] |
//! | Whether a challenge is answered: a server's Basic challenge of a listed environment's origin | [`decide`] |
//! | One answer per view and origin until the credential changes | [`attempts`] |
//! | The credential and its store per project and origin: Credential Manager, Keychain, Secret Service | [`store`], `store_windows`, `store_{macos,linux,codec}` |
//! | The process's service: answer, set, remove; the password registered with `rb_diag` | [`service`] |
//! | `environment.auth` (`Leader E A`, `omitted(secrets)`) | [`defs`], [`command`] |
//! | Whether a credential is stored, and its user - never the password (`getBasicAuthStatus`) | [`status`] |
//! | The answer to CEF's `GetAuthCredentials` (IO thread) | `cef_app/auth.rs` |
//! | The shell's bar: environment, user, password; save or remove | `ui/src/basic-auth/` |
//! | rBrowser's sign-in dialog for every challenge the store does not answer (RB-099-04) | [`prompt`] (the questions), [`ask`] (core and shell), [`later`] (off CEF's IO thread), `AuthPromptBar.vue` |
//!
//! The rules, each held by a test: the credential lives in the platform's store, never in a
//! config file (`basicAuth` refuses any other field); it is bound to the project and to the
//! environment's origin at the time it was set, so an environment pointed elsewhere (MCP may edit
//! `environments`) sends nothing; a proxy, another scheme, another host, port or scheme, a
//! redirect target elsewhere and plain `http` to another machine are never answered; a refused
//! credential is not sent in a loop; MCP can neither read nor set one (`refuse_mcp`, the
//! `config_set` refusal of `basicAuth`); no error repeats what was sent.

pub mod ask;
pub mod attempts;
pub mod command;
pub mod decide;
pub mod defs;
pub mod later;
pub mod origin;
pub mod prompt;
pub mod service;
pub mod status;
pub mod store;
#[cfg(windows)]
mod store_windows;

pub use decide::{Challenge, Refusal};
pub use defs::{AUTH, SECRETS_OMITTED, definitions, refuse_mcp, register};
pub use service::{AuthError, BasicAuth, service};
pub use store::Credential;

/// The settings key the notices link to.
pub const SETTINGS_KEY: &str = "basicAuth.environments";
