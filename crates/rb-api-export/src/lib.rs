//! The API client's exporters and code generators (RB-096-03): a request of `rb_api_model` in,
//! text out - pure, `std` + serde, no file touched and no variable resolved.
//!
//! | Part | Where |
//! | --- | --- |
//! | A request as code: curl, HTTPie, `fetch`, a Playwright test | [`code`]; `curl.rs`, `httpie.rs`, `fetch.rs`, `playwright.rs` |
//! | Requests as a file: a Postman v2.1 collection, one `.http` file, a curl script | [`export`]; `postman.rs`, `collection.rs` |
//! | The plain values of an environment put in, every secret kept a variable | [`fill`] |
//! | A word of a command line, quoted for POSIX shells, PowerShell or `cmd` | `shell.rs` |
//!
//! Secrets: nothing here resolves a variable. The core fills in the plain values of the chosen
//! environment ([`fill`]) before a snippet is made; a secret, a process variable and
//! an unknown name stay variables - `{{name}}` as the file writes it ([`Variables::Template`]),
//! or the environment variable of the shell or script ([`Variables::Environment`]:
//! `"${token}"`, `${env:token}`, `%token%`, `process.env.token`), whose names [`Code::env`]
//! lists.

mod collection;
mod curl;
mod fetch;
#[cfg(test)]
mod fixtures;
mod httpie;
mod js;
mod playwright;
mod postman;
mod shape;
mod shell;
mod template;
mod word;

use rb_api_model::ApiRequest;
use serde::{Deserialize, Serialize};

pub use collection::{Exported, Item, export, file_name, rebase};
pub use template::{fill, fill_text};

/// A language a request is written in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Language {
    Curl,
    Httpie,
    Fetch,
    Playwright,
}

/// The shell a command line is written for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Shell {
    /// sh, bash, zsh - Linux, macOS, WSL, Git Bash.
    Posix,
    /// PowerShell 7.3 or later (`curl.exe`, the arguments passed as written).
    #[serde(rename = "powershell")]
    PowerShell,
    /// The Windows command prompt (`^` before every character it would read).
    Cmd,
}

/// How a variable left in a request shows in the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Variables {
    /// As an environment variable of the shell or script, so the code runs once it is set.
    Environment,
    /// As `{{name}}`, as the request's file writes it.
    Template,
}

/// The form of a file of requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Format {
    /// A Postman collection, v2.1.
    Postman,
    /// One `.http` file, the requests one after the other.
    Http,
    /// A script of curl commands for the shell of [`Options::shell`].
    Curl,
}

/// How code is written.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options {
    pub shell: Shell,
    pub variables: Variables,
}

/// What the code leaves out or changes; the panel names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Note {
    /// OAuth 2 is not in the code: a token is fetched first.
    #[serde(rename = "oauth2")]
    OAuth2,
    /// The language has no digest auth.
    Digest,
    /// `fetch` checks every certificate; the request's TLS check off is not in the code.
    Insecure,
    /// `cmd` holds no line break in an argument; each became a space.
    LineBreaks,
    /// An object keeps one value per name; a repeated header, field or part kept its last.
    DuplicateNames,
    /// The format keeps no timeout of its own.
    Timeout,
}

/// A request as code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Code {
    pub language: Language,
    pub text: String,
    /// The environment variables the code reads ([`Variables::Environment`]), each once.
    pub env: Vec<String>,
    pub notes: Vec<Note>,
}

/// `request` as code in `language`. Run [`fill`] first to put an environment's plain values in.
#[must_use]
pub fn code(request: &ApiRequest, language: Language, options: Options) -> Code {
    let mut words = word::Words::new(options.variables);
    let mut notes = Vec::new();
    let text = match language {
        Language::Curl => curl::command(request, &mut words, options.shell, false, &mut notes),
        Language::Httpie => httpie::command(request, &mut words, options.shell, &mut notes),
        Language::Fetch => fetch::code(request, &mut words, &mut notes),
        Language::Playwright => playwright::code(request, &mut words, &mut notes),
    };
    notes.sort_unstable();
    notes.dedup();
    Code {
        language,
        text,
        env: words.into_env(),
        notes,
    }
}
