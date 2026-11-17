//! Guards the English-only rule (AGENTS.md, Language): code, scripts and documentation are
//! English, user-facing text lives in the locale catalogue behind a key. The exceptions are the
//! job files under `docs/roadmap/jobs/` (archive included), which are German by decision, and the
//! translations of a catalogue (`locales/<lang>.json` other than `en.json`, phase 2).
//!
//! Any German umlaut or sharp s in a covered file is a regression, and so is one of a few German
//! words that need no umlaut. In Markdown an inline code span is a quotation (a status marker of
//! the German job files, such as `entfällt: <reason>`) and is not read.

use std::path::{Path, PathBuf};

use crate::{files_with, relative, workspace_root};

/// Files that must spell German: this one names the characters and words it looks for; the job
/// tools read the German markers of the job files (`gezählt`, `Abnahme`, `Sichtprüfung`).
const ALLOWED_FILES: &[&str] = &[
    "crates/rbrowser/tests/repo_lints/no_german.rs",
    "scripts/lib/archive-jobs.py",
    "scripts/lib/archive_jobs_counts.py",
    "scripts/lib/job_boxes.py",
    "scripts/tests/archive-jobs.sh",
];
/// German by decision: one job per file, written in German (docs/roadmap/jobs/README.md); and
/// captured tool output kept as ADR evidence (a German Windows answers in German).
const GERMAN_DIRS: &[&str] = &["docs/roadmap/jobs/", "docs/adr/0001/evidence/"];
/// The directories read, and the extensions read in them.
const SOURCE_DIRS: &[&str] = &[
    "crates", "scripts", ".claude", "docs", "ui", "agent", "sidecar", "plugins", "sdk",
];
const EXTENSIONS: &[&str] = &[
    ".rs", ".sh", ".py", ".md", ".toml", ".json", ".ts", ".mts", ".js", ".mjs", ".vue", ".wit",
];
const GERMAN_CHARS: &str = "\u{e4}\u{f6}\u{fc}\u{c4}\u{d6}\u{dc}\u{df}";
/// Whole words, case-sensitive: the nouns and verbs a German text about a browser is made of and
/// an English source never spells this way. Short words (`die`, `das`, `und`) are not here; they
/// are English words too.
const GERMAN_WORDS: &[&str] = &[
    "Abbrechen",
    "Ansicht",
    "Bitte",
    "Datei",
    "Dateien",
    "Einstellungen",
    "Fehler",
    "Fenster",
    "Ordner",
    "Passwort",
    "Seite",
    "Speichern",
    "Verbindung",
    "Verzeichnis",
];

fn is_german(line: &str) -> bool {
    line.chars().any(|c| GERMAN_CHARS.contains(c))
        || line
            .split(|c: char| !c.is_alphanumeric())
            .any(|word| GERMAN_WORDS.contains(&word))
}

/// `line` without its inline code spans: every other backtick-delimited stretch is dropped.
fn without_code_spans(line: &str) -> String {
    line.split('`').step_by(2).collect::<Vec<_>>().join(" ")
}

/// Whether `relative` (with `/`) is exempt: an allowed file, a job file, a translation, or a
/// built bundle (`plugins/<id>/**/dist/`, `ui/dist/`): built from sources this lint reads, with
/// the third-party code they ship - Nuxt UI's command palette brings Fuse.js and its table of
/// accented letters.
fn exempt(relative: &str) -> bool {
    let translation = (relative.starts_with("locales/") || relative.contains("/locales/"))
        && !relative.ends_with("/en.json");
    let bundle = (relative.starts_with("plugins/") && relative.contains("/dist/"))
        || relative.starts_with("ui/dist/");
    ALLOWED_FILES.contains(&relative)
        || GERMAN_DIRS.iter().any(|dir| relative.starts_with(dir))
        || translation
        || bundle
}

/// Every covered file: the source directories and the Markdown files at the root.
fn covered(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for directory in SOURCE_DIRS {
        files_with(&root.join(directory), EXTENSIONS, &mut files);
    }
    if let Ok(entries) = std::fs::read_dir(root) {
        files.extend(
            entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md")),
        );
    }
    files
}

#[test]
fn code_and_docs_contain_no_german_text() {
    let root = workspace_root();
    let files = covered(&root);
    assert!(!files.is_empty(), "expected to scan the repository");
    let mut offenders: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let relative = relative(&root, path);
            if exempt(&relative) {
                return None;
            }
            let text = std::fs::read_to_string(path).ok()?;
            let markdown = relative.ends_with(".md");
            let (number, line) = text.lines().enumerate().find(|(_, line)| {
                if markdown {
                    is_german(&without_code_spans(line))
                } else {
                    is_german(line)
                }
            })?;
            Some(format!("{relative}:{}: {}", number + 1, line.trim()))
        })
        .collect();
    offenders.sort();
    assert!(
        offenders.is_empty(),
        "German text found (only docs/roadmap/jobs/ is German):\n{}",
        offenders.join("\n")
    );
}

#[test]
fn a_german_word_without_an_umlaut_is_found_as_a_whole_word() {
    assert!(is_german(r#"        "Datei".to_owned()"#));
    assert!(is_german("// Ordner fehlt"));
    assert!(is_german("// \u{fc}ber"));
    assert!(!is_german(r#"        "folder".to_owned()"#));
    assert!(!is_german("let datei_count = 1; // Dateiname"));
}

#[test]
fn a_quoted_marker_in_markdown_is_not_read() {
    assert!(!is_german(&without_code_spans(
        "ticked with a reason (`entf\u{e4}llt: ...`), never left out."
    )));
    assert!(is_german(&without_code_spans("`code` and Fehler outside")));
}

#[test]
fn only_the_job_files_are_exempt() {
    assert!(exempt("docs/roadmap/jobs/010-01-rb-config.md"));
    assert!(exempt(
        "docs/roadmap/jobs/archive/000-01-repo-infrastruktur.md"
    ));
    assert!(!exempt("docs/roadmap.md"));
    assert!(!exempt("docs/roadmap/jobs.md"));
    assert!(!exempt("AGENTS.md"));
    assert!(exempt("crates/rbrowser/locales/de.json"));
    assert!(!exempt("crates/rbrowser/locales/en.json"));
    assert!(exempt("plugins/framework-devtools/panel/dist/vue.js"));
    assert!(!exempt("plugins/framework-devtools/panel/src/vue.ts"));
    assert!(exempt("ui/dist/index.js"));
    assert!(!exempt("ui/src/main.ts"));
}
