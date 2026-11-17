use super::*;

fn no_analyser(_: &Path, _: Option<&str>) -> Result<Option<ProjectConfig>, String> {
    Ok(None)
}

#[test]
fn init_writes_a_valid_config_and_the_gitignore() {
    let dir = tempfile::tempdir().expect("temp dir");
    let written = init(
        dir.path(),
        Some("http://localhost:5173/"),
        no_analyser,
        |_, _| true,
    )
    .expect("writes");
    assert_eq!(written, ConfigFile::Project.path(dir.path()));
    let gitignore = dir.path().join(rb_config::CONFIG_DIR).join(".gitignore");
    assert_eq!(
        std::fs::read_to_string(gitignore).expect("gitignore"),
        rb_config::GITIGNORE
    );
    let loaded = rb_config::load_project(dir.path()).expect("loads");
    assert!(rb_config::check(&loaded.config).is_empty());
    let ids: Vec<&str> = loaded
        .config
        .breakpoints
        .iter()
        .map(|breakpoint| breakpoint.id.as_str())
        .collect();
    assert_eq!(ids, ["mobile", "desktop"]);
    assert_eq!(
        loaded.config.start_url.as_deref(),
        Some("http://localhost:5173/")
    );
}

#[test]
fn without_a_url_no_start_url_is_written() {
    let dir = tempfile::tempdir().expect("temp dir");
    init(dir.path(), None, no_analyser, |_, _| true).expect("writes");
    let written = rb_config::read_json(&ConfigFile::Project.path(dir.path())).expect("reads");
    assert!(written.get("startUrl").is_none());
}

#[test]
fn an_existing_config_is_never_written_over() {
    let dir = tempfile::tempdir().expect("temp dir");
    init(dir.path(), None, no_analyser, |_, _| true).expect("writes");
    let before = std::fs::read(ConfigFile::Project.path(dir.path())).expect("reads");
    assert!(matches!(
        init(dir.path(), Some("http://x.test"), no_analyser, |_, _| true),
        Err(InitError::Exists(_))
    ));
    let after = std::fs::read(ConfigFile::Project.path(dir.path())).expect("reads");
    assert_eq!(before, after);
}

#[test]
fn nothing_is_written_without_a_yes() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(
        init(dir.path(), None, no_analyser, |_, _| false),
        Err(InitError::NotConfirmed)
    ));
    assert!(!dir.path().join(rb_config::CONFIG_DIR).exists());
}

#[test]
fn the_analysers_proposal_is_written_when_it_has_one() {
    let dir = tempfile::tempdir().expect("temp dir");
    let proposal = |_: &Path, url: Option<&str>| {
        let mut config = crate::host::state::default_config();
        config.start_url = url.map(str::to_owned);
        Ok(Some(config))
    };
    init(
        dir.path(),
        Some("http://localhost:3000"),
        proposal,
        |_, _| true,
    )
    .expect("writes");
    let loaded = rb_config::load_project(dir.path()).expect("loads");
    assert_eq!(loaded.config.breakpoints.len(), 3);
}

#[test]
fn a_failing_analyser_falls_back_to_the_minimal_config() {
    let dir = tempfile::tempdir().expect("temp dir");
    let failing = |_: &Path, _: Option<&str>| Err("no browser".to_owned());
    init(dir.path(), None, failing, |_, _| true).expect("writes");
    let loaded = rb_config::load_project(dir.path()).expect("loads");
    assert_eq!(loaded.config.breakpoints.len(), 2);
}

#[test]
fn a_url_outside_the_allowed_schemes_is_refused() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(
        init(
            dir.path(),
            Some("javascript:alert(1)"),
            no_analyser,
            |_, _| true
        ),
        Err(InitError::Url(_))
    ));
}

/// A project's start URL is `http`/`https` only (charge-4 review, finding 2): a `file:` URL
/// opens with `rbrowser open`, but `init` does not write it into the shared config.
#[test]
fn a_file_url_is_no_start_url() {
    let dir = tempfile::tempdir().expect("temp dir");
    assert!(matches!(
        init(
            dir.path(),
            Some("file:///C:/site/index.html"),
            no_analyser,
            |_, _| true
        ),
        Err(InitError::Url(_))
    ));
    assert!(!ConfigFile::Project.path(dir.path()).exists());
}

#[test]
fn the_proposal_says_where_it_comes_from() {
    let dir = tempfile::tempdir().expect("temp dir");
    let proposal = |_: &Path, _: Option<&str>| Ok(Some(crate::host::state::default_config()));
    let mut seen = None;
    init(dir.path(), None, proposal, |_, source| {
        seen = Some(source);
        false
    })
    .expect_err("not confirmed");
    assert_eq!(seen, Some(Source::Analyser));

    let failing = |_: &Path, _: Option<&str>| Err("no stylesheet".to_owned());
    init(dir.path(), None, failing, |_, source| {
        seen = Some(source);
        false
    })
    .expect_err("not confirmed");
    assert_eq!(seen, Some(Source::Default));
}

/// What `ask` prints for `config` and the answer `typed`, and whether it agreed.
fn asked(config: &Value, source: Source, typed: &str) -> (String, bool) {
    let mut output = Vec::new();
    let agreed = ask(config, source, &mut typed.as_bytes(), &mut output);
    (String::from_utf8(output).expect("text"), agreed)
}

#[test]
fn the_prompt_lists_every_view_and_asks_before_writing() {
    let config = minimal_config(Some("https://degoya.digital/"));
    let (text, agreed) = asked(&config, Source::Default, "y\n");
    assert!(agreed);
    let lines: Vec<&str> = text.lines().collect();
    assert!(lines[0].contains("2 views"), "{text}");
    assert!(lines[0].contains("https://degoya.digital/"), "{text}");
    let mobile = lines
        .iter()
        .find(|line| line.contains("mobile"))
        .expect("mobile");
    assert!(
        mobile.contains("393 x 852") && mobile.contains("default"),
        "{mobile}"
    );
    let desktop = lines
        .iter()
        .find(|line| line.contains("desktop"))
        .expect("desktop");
    assert!(
        desktop.contains("1440 x 900") && desktop.contains("default"),
        "{desktop}"
    );
    let config_path = ConfigFile::Project.path(Path::new(""));
    assert!(
        text.ends_with(&format!("Write {}? [y/N] ", config_path.display())),
        "{text}"
    );
}

#[test]
fn the_analysers_views_are_named_as_its_own() {
    let config = serde_json::to_value(crate::host::state::default_config()).expect("json");
    let (text, _) = asked(&config, Source::Analyser, "n\n");
    let views: Vec<&str> = text.lines().filter(|line| line.starts_with("  ")).collect();
    assert_eq!(views.len(), 3, "{text}");
    assert!(
        views.iter().all(|line| line.ends_with("analyser")),
        "{text}"
    );
}

#[test]
fn a_view_sized_by_its_device_names_the_device() {
    let config = json!({ "version": 1, "breakpoints": [{ "id": "phone", "device": "iphone-15" }] });
    let (text, _) = asked(&config, Source::Analyser, "");
    assert!(text.contains("device iphone-15"), "{text}");
}

#[test]
fn only_a_yes_writes() {
    let config = minimal_config(None);
    for typed in ["y\n", "Y\n", "yes\n", " YES \r\n"] {
        assert!(asked(&config, Source::Default, typed).1, "{typed:?}");
    }
    for typed in ["", "\n", "n\n", "no\n", "yy\n", "ja\n"] {
        assert!(!asked(&config, Source::Default, typed).1, "{typed:?}");
    }
}
