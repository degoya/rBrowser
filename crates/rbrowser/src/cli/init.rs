//! `rbrowser init [--url URL] [--yes]`: writes `.rbrowser/config.json` and
//! `.rbrowser/.gitignore` into the project folder (`--project`, else the working folder). The
//! breakpoints are the analyser's proposal (`analyzer::init_config`, RB-010-13) or, without
//! one, a mobile and a desktop view. Needs no running instance; an existing config is never
//! written over.

use std::io::{BufRead, IsTerminal as _, Write};
use std::path::{Path, PathBuf};

use rb_config::{ConfigFile, ProjectConfig};
use serde_json::{Map, Value, json};

use super::exit;

/// Why `init` wrote nothing.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    /// The project has a config already.
    #[error("{0} exists already; edit it or delete it first")]
    Exists(PathBuf),
    /// The start URL is not allowed.
    #[error("{0}")]
    Url(String),
    /// The user did not confirm.
    #[error("nothing was written")]
    NotConfirmed,
    /// Writing failed.
    #[error(transparent)]
    Write(#[from] rb_config::ConfigError),
}

impl InitError {
    /// The exit code of this failure.
    pub fn exit_code(&self) -> i32 {
        match self {
            InitError::Exists(_) => exit::CONFIG_EXISTS,
            InitError::Url(_) => exit::USAGE,
            InitError::NotConfirmed => exit::NOT_CONFIRMED,
            InitError::Write(_) => exit::FAILED,
        }
    }
}

/// The config without the analyser: a mobile and a desktop view, the start URL when given.
pub fn minimal_config(url: Option<&str>) -> Value {
    let mut config = Map::new();
    config.insert("version".to_owned(), json!(rb_config::CURRENT_VERSION));
    if let Some(url) = url {
        config.insert("startUrl".to_owned(), json!(url));
    }
    config.insert(
        "breakpoints".to_owned(),
        json!([
            { "id": "mobile", "label": "Mobile", "width": 393, "height": 852, "dpr": 3, "mobile": true, "touch": true },
            { "id": "desktop", "label": "Desktop", "width": 1440, "height": 900 }
        ]),
    );
    Value::Object(config)
}

/// Where the proposed views come from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// The breakpoint analyser read the site (RB-010-13).
    Analyser,
    /// The analyser had no proposal: a mobile and a desktop view ([`minimal_config`]).
    Default,
}

impl Source {
    /// The word in the list `init` shows.
    fn word(self) -> &'static str {
        match self {
            Source::Analyser => "analyser",
            Source::Default => "default",
        }
    }
}

/// Writes the config of the project at `root`: `analyser`'s proposal for `url`, or the
/// [`minimal_config`] when it has none or fails; nothing unless `confirm` agrees to the text
/// and its [`Source`].
pub fn init(
    root: &Path,
    url: Option<&str>,
    analyser: impl FnOnce(&Path, Option<&str>) -> Result<Option<ProjectConfig>, String>,
    confirm: impl FnOnce(&Value, Source) -> bool,
) -> Result<PathBuf, InitError> {
    let path = ConfigFile::Project.path(root);
    if path.exists() {
        return Err(InitError::Exists(path));
    }
    let url = url
        .map(crate::instance::protocol::open_url)
        .transpose()
        .map_err(InitError::Url)?;
    // The config is shared with the team: its `startUrl` is `http`/`https` only
    // (`rb_config::check`), a `file:` URL opens with `rbrowser open` instead.
    if let Some(url) = url.as_deref()
        && !rb_config::bookmarks::is_web_url(url)
    {
        return Err(InitError::Url(format!(
            "{url}: a project's start URL must be an http or https URL; open a file with \
             `rbrowser open` instead"
        )));
    }
    let proposal = analyser(root, url.as_deref()).unwrap_or_else(|error| {
        tracing::warn!(%error, "the analyser had no proposal; writing the minimal config");
        eprintln!("rbrowser init: the analyser had no proposal ({error}); writing a mobile and a desktop view");
        None
    });
    let (config, source) = match proposal.map(|config| serde_json::to_value(&config)) {
        Some(Ok(value)) => (value, Source::Analyser),
        Some(Err(error)) => {
            tracing::warn!(%error, "the analyser's proposal cannot be written; writing the minimal config");
            (minimal_config(url.as_deref()), Source::Default)
        }
        None => (minimal_config(url.as_deref()), Source::Default),
    };
    if !confirm(&config, source) {
        return Err(InitError::NotConfirmed);
    }
    rb_config::write_config_file(root, ConfigFile::Project, &config)?;
    Ok(path)
}

/// Shows the views of `config` on `output` - id, width x height (or the device that sizes it),
/// [`Source`] - asks whether to write the config and reads the answer from `input`: only `y` or
/// `yes` (any case) agrees; an empty line or the end of the input does not.
pub fn ask(
    config: &Value,
    source: Source,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> bool {
    let views = config
        .get("breakpoints")
        .and_then(Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let header = match config.get("startUrl").and_then(Value::as_str) {
        Some(url) => format!("rbrowser init: {} views for {url}:", views.len()),
        None => format!("rbrowser init: {} views:", views.len()),
    };
    let mut text = format!("{header}\n");
    for view in views {
        let text_of = |key: &str| view.get(key).and_then(Value::as_str).unwrap_or_default();
        let number = |key: &str| view.get(key).and_then(Value::as_u64);
        let size = match (number("width"), number("height"), text_of("device")) {
            (Some(width), Some(height), _) => format!("{width} x {height}"),
            (_, _, device) if !device.is_empty() => format!("device {device}"),
            (width, height, _) => format!(
                "{} x {}",
                width.map_or("?".to_owned(), |width| width.to_string()),
                height.map_or("?".to_owned(), |height| height.to_string())
            ),
        };
        text.push_str(&format!(
            "  {:<14} {size:<20} {}\n",
            text_of("id"),
            source.word()
        ));
    }
    text.push_str(&format!(
        "Write {}? [y/N] ",
        ConfigFile::Project.path(Path::new("")).display()
    ));
    if output
        .write_all(text.as_bytes())
        .and_then(|()| output.flush())
        .is_err()
    {
        return false;
    }
    let mut answer = String::new();
    if input.read_line(&mut answer).is_err() || answer.is_empty() {
        // The end of the input: the prompt's line is ended for the next output.
        let _ = writeln!(output);
        return false;
    }
    let answer = answer.trim();
    answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes")
}

/// `rbrowser init` in the folder `root`: asks on the terminal unless `yes`; returns the exit code.
pub fn run(root: &Path, url: Option<&str>, yes: bool) -> i32 {
    if !root.is_dir() {
        eprintln!("rbrowser init: {} is not a folder", root.display());
        return exit::NO_PROJECT;
    }
    // `init` runs before the core exists: the analyser's plugin calls need a runtime on this
    // thread (RB-080-28 refuses to build one per call), and there is no UI thread to hang.
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("rbrowser init: no runtime: {error}");
            return exit::NO_PROJECT;
        }
    };
    let _entered = runtime.enter();
    let confirm = |config: &Value, source: Source| {
        if yes {
            return true;
        }
        // rbrowser.com hands the terminal over (`console`); a pipe, a file or a shell's console
        // that rbrowser.exe was started from straight away are no terminal to ask on.
        if !std::io::stdin().is_terminal() {
            eprintln!("rbrowser init: pass --yes to write the config without asking");
            return false;
        }
        ask(
            config,
            source,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout(),
        )
    };
    match init(root, url, crate::analyzer::init_config, confirm) {
        Ok(path) => {
            println!("{}", path.display());
            exit::OK
        }
        Err(error) => {
            eprintln!("rbrowser init: {error}");
            error.exit_code()
        }
    }
}

#[cfg(test)]
#[path = "init_tests.rs"]
mod tests;
