//! The API client's importers (RB-096-02): the collections of other tools read - never changed -
//! into [`rb_api_model`] requests and environments, with a [`Report`] of what was taken, what a
//! format has that rBrowser does not (scripts, tests, other auth kinds: reported) and what was
//! left out (with the reason). Pure: texts in, an [`Import`] out; the core writes the files.
//!
//! | Format | Module |
//! | --- | --- |
//! | Postman collection v2.0/v2.1, environment, globals | [`postman`] |
//! | Insomnia export v4, JSON and YAML | [`insomnia`] |
//! | Thunder Client collection and environment | [`thunder`] |
//! | Bruno `.bru` folder (`bruno.json`, `collection.bru`, `environments/*.bru`) | [`bruno`] |
//! | HTTPie desktop export (collection, workspace) | [`httpie`] |
//! | `.http` files of several requests (VS Code REST Client, JetBrains), `http-client.env.json` | [`http_files`] |
//! | OpenAPI 3.x and Swagger 2.0, JSON and YAML: one request per operation, an example body | [`openapi`] |
//! | HAR | [`har`] |
//! | A pasted `curl` command (bash, cmd, PowerShell copies) | [`curl`] |
//!
//! Secrets: a literal token, password, API key, cookie or client secret never reaches a request
//! or an environment value - it becomes `{{$secret <name>}}` (the process variable
//! `RB_SECRET_<NAME>`, or the private value `<name>`), and the report names each placeholder,
//! never the value ([`secrets`]).

pub mod bruno;
mod bruno_file;
pub mod collect;
mod common;
pub mod curl;
mod curl_words;
pub mod detect;
pub mod har;
pub mod http_files;
pub mod httpie;
pub mod insomnia;
mod json;
pub mod openapi;
mod openapi_auth;
mod openapi_example;
pub mod postman;
pub mod report;
pub mod secrets;
pub mod thunder;
mod yaml;

pub use collect::{Collector, Environment, Import, Imported, Variable};
pub use detect::{Format, detect};
pub use report::{Note, NoteKind, Report, SecretNote};

/// The most text one import reads, in bytes (all files together).
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;
/// The most files one import reads.
pub const MAX_SOURCES: usize = 500;

/// One file of an import, or the pasted text (`name` empty).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// The file's name, a folder's files with their relative path (`users/get.bru`).
    pub name: String,
    pub text: String,
}

impl Source {
    /// A file `name` with `text`.
    #[must_use]
    pub fn new(name: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            text: text.into(),
        }
    }
}

/// Why nothing was imported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// No file, or only empty ones.
    Empty,
    /// Over [`MAX_INPUT_BYTES`] or [`MAX_SOURCES`].
    TooLarge,
    /// No format recognised in `source`.
    Unknown { source: String },
    /// `source` is of its format but does not read.
    Unreadable { source: String, reason: String },
}

impl ImportError {
    /// The reason's id: `empty`, `too_large`, `unknown_format`, `unreadable`.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::TooLarge => "too_large",
            Self::Unknown { .. } => "unknown_format",
            Self::Unreadable { .. } => "unreadable",
        }
    }
}

fn unreadable(source: &Source, reason: impl Into<String>) -> ImportError {
    ImportError::Unreadable {
        source: shown_name(source),
        reason: reason.into(),
    }
}

/// The name a report shows for `source`: its file name, `pasted text` for none.
fn shown_name(source: &Source) -> String {
    if source.name.trim().is_empty() {
        "pasted text".to_owned()
    } else {
        source.name.chars().take(200).collect()
    }
}

/// Reads `sources` - each in its own format, or all in `format` - into one import. A Bruno
/// collection's files are read together; the other files each on their own (a Postman
/// collection with its environments, an `.http` file with `http-client.env.json`).
///
/// # Errors
///
/// [`ImportError`]: nothing to read, too much, a file of no known format, a file that does not
/// read as its format.
pub fn import(sources: &[Source], format: Option<Format>) -> Result<Import, ImportError> {
    let sources: Vec<&Source> = sources
        .iter()
        .filter(|source| !source.text.trim().is_empty())
        .collect();
    if sources.is_empty() {
        return Err(ImportError::Empty);
    }
    let total: usize = sources.iter().map(|source| source.text.len()).sum();
    if total > MAX_INPUT_BYTES || sources.len() > MAX_SOURCES {
        return Err(ImportError::TooLarge);
    }
    // A folder that holds a `.bru` file is a Bruno collection, its other files included.
    let folder = sources
        .iter()
        .any(|source| source.name.to_ascii_lowercase().ends_with(".bru"));
    let format = if format.is_none() && folder {
        Some(Format::Bruno)
    } else {
        format
    };
    let mut collector = Collector::default();
    let mut bruno = Vec::new();
    for source in sources {
        let found = match format {
            Some(format) => format,
            None => detect(source).ok_or_else(|| ImportError::Unknown {
                source: shown_name(source),
            })?,
        };
        collector.format(found);
        let result = match found {
            Format::Bruno => {
                bruno.push(source.clone());
                Ok(())
            }
            Format::Postman => postman::read(source, &mut collector),
            Format::Insomnia => insomnia::read(source, &mut collector),
            Format::ThunderClient => thunder::read(source, &mut collector),
            Format::Httpie => httpie::read(source, &mut collector),
            Format::Http => http_files::read(source, &mut collector),
            Format::OpenApi => openapi::read(source, &mut collector),
            Format::Har => har::read(source, &mut collector),
            Format::Curl => curl::read(source, &mut collector),
        };
        result.map_err(|reason| unreadable(source, reason))?;
    }
    if !bruno.is_empty() {
        bruno::read(&bruno, &mut collector)
            .map_err(|reason| unreadable(&Source::new("bruno", ""), reason))?;
    }
    Ok(collector.finish())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
