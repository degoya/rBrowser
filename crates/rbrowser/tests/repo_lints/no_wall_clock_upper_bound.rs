//! No test asserts an upper bound on the wall clock (AGENTS.md, Conventions). A test that says
//! `started.elapsed() < 200 ms` goes red under load although the logic is right; the statement
//! is kept with another instrument: a give-up deadline (`tokio::time::timeout`, a bounded poll),
//! a paused runtime (`#[tokio::test(start_paused = true)]`), or the order of calls where "B does
//! not wait for C" is meant. Lower bounds (a debounce, a timeout really waited for) stay.
//!
//! The lint reads the test files - Rust `*_tests.rs`, `tests.rs` and everything under a `tests/`
//! directory, TypeScript `*.test.ts` of the front ends - without their comments and texts
//! (`scan::code_only`), and reports:
//!
//! * an `assert!` whose statement compares `.elapsed()` or `Instant::now() - <start>` upward
//!   (`<`, `<=`), compares anything upward against a `Duration::` (`p95 <
//!   Duration::from_millis(300)`), compares a `.as_millis()`, `.as_secs()` or `.as_micros()`
//!   upward, or compares upward a name the file bound to a measured span (`let took =
//!   started.elapsed();`, the initialiser also on the next line);
//! * an `expect(...)` chain ending in `toBeLessThan(` / `toBeLessThanOrEqual(` whose argument
//!   is a `Date.now()` / `performance.now()` difference, or a name bound to one.
//!
//! A measurement with orders of magnitude of headroom carries `// wall-clock: allow (<reason>)`
//! on its line or the line above.

use std::path::Path;

use crate::scan::{calls, code_only, line_of, statement_from};
use crate::{files_with, relative, workspace_root};

/// The marker of an upper bound kept on purpose: `// wall-clock: allow (<reason>)`.
const ALLOW: &str = "wall-clock: allow";

/// Where the TypeScript tests live; a `dist/` below them is build output.
const WEB_DIRS: &[&str] = &["ui", "agent", "sidecar", "plugins"];

/// What a span bound to a name is measured from, in Rust and in TypeScript.
const SPAN_SOURCES: &[&str] = &[
    ".elapsed()",
    "Instant::now() -",
    ".duration_since(",
    "Date.now() -",
    "performance.now() -",
];

/// A span's unit conversions: `elapsed.as_millis() < 200` is an upper bound without `.elapsed()`
/// or a `Duration::` in the statement.
const SPAN_UNITS: &[&str] = &[".as_millis()", ".as_secs()", ".as_micros()"];

/// What a TypeScript `expect(...)` argument measures directly.
const WEB_CLOCKS: &[&str] = &["Date.now()", "performance.now()", "hrtime"];

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether `relative` (with `/`) is a Rust test file: a sibling `*_tests.rs` or `tests.rs`, or
/// anything under a `tests/` directory.
fn is_rust_test_file(relative: &str) -> bool {
    let name = relative.rsplit('/').next().unwrap_or(relative);
    name.ends_with("_tests.rs")
        || name == "tests.rs"
        || relative.split('/').any(|part| part == "tests")
}

/// Whether `relative` (with `/`) is a front-end test file, not in a `dist/`.
fn is_web_test_file(relative: &str) -> bool {
    (relative.ends_with(".test.ts") || relative.ends_with(".test.mts"))
        && !relative.split('/').any(|part| part == "dist")
}

/// The names `code` binds to a measured span: `let took = started.elapsed();`,
/// `const took = Date.now() - started;`; an initialiser the formatter wrapped onto the next
/// line counts as well.
fn span_names(code: &str) -> Vec<String> {
    let lines: Vec<&str> = code.lines().collect();
    lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.trim_start();
            let binding = ["let ", "const ", "var "]
                .iter()
                .find_map(|keyword| line.strip_prefix(keyword))?;
            let (name, value) = binding.split_once('=')?;
            let name = name.strip_prefix("mut ").unwrap_or(name);
            let name = name.split(':').next()?.trim();
            let value = if value.trim().is_empty() {
                lines.get(index + 1).copied().unwrap_or_default()
            } else {
                value
            };
            let measured = SPAN_SOURCES.iter().any(|source| value.contains(source));
            (measured && !name.is_empty() && name.chars().all(is_ident)).then(|| name.to_owned())
        })
        .collect()
}

/// Whether `text` compares `name` upward: `name <` or `name <=`, the name whole.
fn compares_upward(text: &str, name: &str) -> bool {
    text.match_indices(name).any(|(at, _)| {
        let whole = text[..at].chars().next_back().is_none_or(|c| !is_ident(c));
        let after = text[at + name.len()..].trim_start();
        whole && after.starts_with('<') && !after.starts_with("<<")
    })
}

/// Whether `text` has `needle` followed by `<` or `<=` (not `<<`).
fn followed_upward(text: &str, needle: &str) -> bool {
    text.match_indices(needle).any(|(at, _)| {
        let after = text[at + needle.len()..].trim_start();
        after.starts_with('<') && !after.starts_with("<<")
    })
}

/// Whether the `assert!` statement `statement` is an upper bound on a span.
fn is_rust_upper_bound(statement: &str, names: &[String]) -> bool {
    let upward = statement.contains(" < ") || statement.contains(" <= ");
    let measured = statement.contains(".elapsed()") || statement.contains("Instant::now() -");
    (measured && upward)
        || statement.contains(" < Duration::")
        || statement.contains(" <= Duration::")
        || SPAN_UNITS
            .iter()
            .any(|unit| followed_upward(statement, unit))
        || names.iter().any(|name| compares_upward(statement, name))
}

/// Whether the `expect(...)` chain `chain` with the argument `argument` is an upper bound.
fn is_web_upper_bound(chain: &str, argument: &str, names: &[String]) -> bool {
    (chain.contains(".toBeLessThan(") || chain.contains(".toBeLessThanOrEqual("))
        && (WEB_CLOCKS.iter().any(|clock| argument.contains(clock))
            || names.iter().any(|name| {
                argument
                    .split(|c: char| !is_ident(c))
                    .any(|word| word == name)
            }))
}

/// Whether the statement starting on line `line` (1-based) carries the allow marker on its own
/// line or the one above.
fn allowed(raw: &[&str], line: usize) -> bool {
    let index = line.saturating_sub(1);
    raw.get(index).is_some_and(|text| text.contains(ALLOW))
        || (index > 0 && raw[index - 1].contains(ALLOW))
}

/// The 1-based lines of `source` on which a Rust `assert!` bounds a span upward.
fn rust_upper_bounds(source: &str) -> Vec<usize> {
    let raw: Vec<&str> = source.lines().collect();
    let code = code_only(source);
    let names = span_names(&code);
    calls(&code, "assert!(")
        .into_iter()
        .map(|at| (line_of(&code, at), statement_from(&code, at)))
        .filter(|(line, statement)| is_rust_upper_bound(statement, &names) && !allowed(&raw, *line))
        .map(|(line, _)| line)
        .collect()
}

/// The 1-based lines of `source` on which an `expect(...)` bounds a span upward.
fn web_upper_bounds(source: &str) -> Vec<usize> {
    let raw: Vec<&str> = source.lines().collect();
    let code = code_only(source);
    let names = span_names(&code);
    calls(&code, "expect(")
        .into_iter()
        .map(|at| {
            let chain = statement_from(&code, at);
            let argument = statement_from(&code, at + "expect(".len());
            (line_of(&code, at), chain, argument)
        })
        .filter(|(line, chain, argument)| {
            is_web_upper_bound(chain, argument, &names) && !allowed(&raw, *line)
        })
        .map(|(line, _, _)| line)
        .collect()
}

/// Every covered test file of the repository with its scan, relative to the root.
/// A test file's path and the scan that reads its language.
type Scanned = (String, fn(&str) -> Vec<usize>);

fn test_files(root: &Path) -> Vec<Scanned> {
    let mut rust = Vec::new();
    files_with(root, &[".rs"], &mut rust);
    let mut web = Vec::new();
    for directory in WEB_DIRS {
        files_with(&root.join(directory), &[".ts", ".mts"], &mut web);
    }
    let rust = rust
        .iter()
        .map(|path| relative(root, path))
        .filter(|path| is_rust_test_file(path))
        .map(|path| (path, rust_upper_bounds as fn(&str) -> Vec<usize>));
    let web = web
        .iter()
        .map(|path| relative(root, path))
        .filter(|path| is_web_test_file(path))
        .map(|path| (path, web_upper_bounds as fn(&str) -> Vec<usize>));
    rust.chain(web).collect()
}

#[test]
fn no_test_asserts_an_upper_bound_on_the_wall_clock() {
    let root = workspace_root();
    let files = test_files(&root);
    assert!(
        files.iter().any(|(path, _)| path.ends_with("_tests.rs"))
            && files.iter().any(|(path, _)| path.ends_with(".test.ts")),
        "expected Rust and TypeScript test files"
    );
    let mut found = Vec::new();
    for (path, scan) in files {
        let source = std::fs::read_to_string(root.join(&path)).expect("read test file");
        found.extend(
            scan(&source)
                .into_iter()
                .map(|line| format!("{path}:{line}")),
        );
    }
    assert!(
        found.is_empty(),
        "a test asserts an upper bound on the wall clock and goes red under load; keep the \
         statement with a give-up deadline, a paused runtime or the order of calls \
         (or mark it `// {ALLOW} (<reason>)`):\n{}",
        found.join("\n")
    );
}

/// The Rust scan: `elapsed()` compared upward, a `Duration::` upper bound and a bound span
/// are found; a poll loop's deadline, a lower bound, a comment and an allowed line are not.
#[test]
fn the_scan_finds_a_rust_upper_bound() {
    let source = r#"fn direct() {
    let started = Instant::now();
    assert!(started.elapsed() < Duration::from_millis(200));
    assert!(
        started.elapsed() <= Duration::from_millis(100),
        "B did not wait for C"
    );
}

fn bound_span() {
    let took = started.elapsed();
    assert!(took < SETTLE * 2, "settled after {took:?}");
    let p95 = times[94];
    assert!(p95 < Duration::from_millis(300), "{what} p95 {p95:?}");
}

fn kept() {
    while started.elapsed() < limit {
        poll();
    }
    assert!(started.elapsed() >= Duration::from_millis(30), "debounced");
    // assert!(started.elapsed() < Duration::from_secs(1));
    let text = "assert!(started.elapsed() < Duration::from_secs(1))";
    assert!(rows.len() < 60);
    assert!(stook < 3, "another name");
    // wall-clock: allow (a p95 over 100 rounds with 300x headroom)
    assert!(p95 < Duration::from_millis(300));
    assert!(took >= SETTLE);
}
"#;
    assert_eq!(rust_upper_bounds(source), vec![3, 4, 12, 14]);
    assert_eq!(span_names(&code_only(source)), vec!["took"]);
    assert_eq!(
        span_names("let mut a: Duration = Instant::now() - t0;\nconst b = Date.now() - t0;"),
        vec!["a", "b"]
    );
}

/// The TypeScript scan: a `Date.now()` / `performance.now()` difference or a name bound to one
/// under `toBeLessThan` is found; another number, a lower bound and an allowed line are not.
#[test]
fn the_scan_finds_a_web_upper_bound() {
    let source = r#"it('does not wait', async () => {
  const started = Date.now();
  await run();
  expect(Date.now() - started).toBeLessThan(1000);
  const took = performance.now() - t0;
  expect(took).toBeLessThanOrEqual(5);
  expect(rows(viewer).length).toBeLessThan(60);
  expect(Date.now() - started).toBeGreaterThanOrEqual(30);
  // expect(Date.now() - started).toBeLessThan(1000);
  expect(tookOther).toBeLessThan(5);
  // wall-clock: allow (an idle loop)
  expect(Date.now() - started).toBeLessThan(10_000);
});
"#;
    assert_eq!(web_upper_bounds(source), vec![4, 6]);
}

/// The Rust scan also finds a span computed inline without `.elapsed()` or a `Duration::` on
/// the right (`Instant::now() - started < limit`, `elapsed.as_millis() < 200`) and a `let`
/// whose initialiser is on the next line; lower bounds of the same forms are not found.
#[test]
fn the_scan_finds_an_inline_span_and_a_wrapped_binding() {
    let source = r#"fn inline() {
    assert!(Instant::now() - started < limit);
    assert!(elapsed.as_millis() < 200);
    assert!(elapsed.as_secs() <= 1, "{elapsed:?}");
    assert!(elapsed.as_micros() < 5_000);
    assert!(Instant::now() - started >= limit);
    assert!(elapsed.as_millis() >= 10);
    assert!(rows.len() < 60);
}

fn wrapped() {
    let took =
        started.elapsed();
    let mut other: Duration =
        Instant::now() - t0;
    assert!(took < SETTLE * 2, "settled after {took:?}");
    assert!(other <= SETTLE);
    assert!(took >= SETTLE);
}
"#;
    assert_eq!(rust_upper_bounds(source), vec![2, 3, 4, 5, 16, 17]);
    assert_eq!(span_names(&code_only(source)), vec!["took", "other"]);
}

#[test]
fn the_covered_files_are_the_test_files() {
    for path in [
        "crates/rb-sync/src/hub_tests.rs",
        "crates/rb-sync/src/tests.rs",
        "crates/rbrowser/tests/repo_lints/scan.rs",
        "plugins/seo/src/report/html_tests.rs",
    ] {
        assert!(is_rust_test_file(path), "{path}");
    }
    for path in ["crates/rb-sync/src/hub.rs", "crates/rb-sync/src/testing.rs"] {
        assert!(!is_rust_test_file(path), "{path}");
    }
    assert!(is_web_test_file("sidecar/src/modules/flows/runner.test.ts"));
    assert!(is_web_test_file(
        "plugins/overlays/web/e2e/overlays.e2e.test.ts"
    ));
    assert!(!is_web_test_file("sidecar/dist/runner.test.ts"));
    assert!(!is_web_test_file("ui/src/layout/minimap.ts"));
}
