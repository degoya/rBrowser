use rb_plugin_api::test_support::MemoryHost;

use super::Settings;
use crate::clock::Format;

#[test]
fn without_settings_every_todo_file_outside_node_modules_and_git() {
    let settings = Settings::read(&mut MemoryHost::new());
    assert_eq!(settings, Settings::default());
    assert!(settings.wanted("TODO.todo"));
    assert!(settings.wanted("docs/plan.todo"));
    assert!(!settings.wanted("notes.txt"));
    assert!(settings.excluded("node_modules"));
    assert!(settings.excluded(".GIT"));
    // Build output is left out too (code review RB-110-13).
    for folder in ["target", "dist", "build", "out", ".next", ".nuxt", "vendor"] {
        assert!(settings.excluded(folder), "{folder}");
    }
    assert!(!settings.excluded("docs"));
    assert_eq!(settings.format, Format::TodoPlus);
}

#[test]
fn the_settings_narrow_the_files_and_pick_the_timestamp() {
    let mut host = MemoryHost::new();
    host.settings
        .insert("files".to_owned(), r#"["docs/*.todo", "", 3]"#.to_owned());
    host.settings
        .insert("exclude".to_owned(), r#"["vendor"]"#.to_owned());
    host.settings
        .insert("timestampFormat".to_owned(), r#""fullYear""#.to_owned());
    let settings = Settings::read(&mut host);
    assert_eq!(settings.files, ["docs/*.todo"]);
    assert!(!settings.wanted("TODO.todo"));
    assert!(settings.excluded("vendor"));
    assert!(!settings.excluded("node_modules"));
    assert_eq!(settings.format, Format::FullYear);

    // An empty list of files, or values that do not read, keep the defaults.
    let mut host = MemoryHost::new();
    host.settings.insert("files".to_owned(), "[]".to_owned());
    host.settings
        .insert("timestampFormat".to_owned(), r#""YY""#.to_owned());
    assert_eq!(Settings::read(&mut host), Settings::default());
}

#[test]
fn a_glob_names_the_folders_before_its_first_wildcard() {
    let settings = Settings {
        files: vec![".plans/*.todo".to_owned(), "docs/team/**/*.todo".to_owned()],
        ..Settings::default()
    };
    assert!(settings.names_folder(".plans"));
    assert!(settings.names_folder("docs"));
    assert!(settings.names_folder("docs/team"));
    assert!(!settings.names_folder("docs/team/a"));
    assert!(!settings.names_folder(".plan"));
    assert!(!Settings::default().names_folder("docs"));
}
