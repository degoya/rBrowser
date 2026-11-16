use rb_config::{ConfigFile, Environment};
use rb_engine::ViewHost;
use serde_json::{Value, json};

use super::{INVALID_KEY, KEPT_ABSOLUTE_KEY, MADE_RELATIVE_KEY, NONE_RELATIVE_KEY, write_all};
use crate::bookmarks::{ADD, MAKE_RELATIVE, OPEN, run, summaries};
use crate::sync_hub::tests::{Setup, setup};

fn environment(id: &str, url: &str) -> Environment {
    Environment {
        id: id.to_owned(),
        url: url.to_owned(),
        color: None,
        reload_interval_sec: None,
    }
}

fn without_nulls(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|_, item| !item.is_null());
            map.values_mut().for_each(without_nulls);
        }
        Value::Array(items) => items.iter_mut().for_each(without_nulls),
        _ => {}
    }
}

/// A project on disk with `local`, `staging` and `production`, the bookmarks given, its
/// `config.json` the config in effect and `local` the environment in effect.
fn project(setup: &Setup, bookmarks: Value) -> tempfile::TempDir {
    let environments = vec![
        environment("local", "http://localhost:5173"),
        environment("staging", "https://staging.example.com/"),
        environment("production", "https://www.example.com"),
    ];
    project_with(setup, environments, bookmarks)
}

fn project_with(
    setup: &Setup,
    environments: Vec<Environment>,
    bookmarks: Value,
) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("temp dir");
    let host = setup.core.host();
    let mut config = host.config();
    config.start_url = Some("http://localhost:5173/".to_owned());
    config.environments = environments;
    config.bookmarks = serde_json::from_value(bookmarks).expect("bookmarks");
    let mut document = serde_json::to_value(&config).expect("a document");
    without_nulls(&mut document);
    rb_config::write_config_file(dir.path(), ConfigFile::Project, &document).expect("written");
    host.open_config(config, Some(dir.path().to_path_buf()))
        .expect("opens");
    let first = host.config().environments.first().map(|env| env.id.clone());
    host.update_shell_state(|shell| shell.environment = first);
    dir
}

/// The active view shows `url`.
fn showing(setup: &Setup, url: &str) {
    let host = setup.core.host();
    let view = host.views().first().expect("a view").id;
    host.set_active_view(view).expect("a view");
    host.on_address_change(view, url);
}

fn stored(dir: &tempfile::TempDir, file: ConfigFile) -> Value {
    rb_config::read_json(&file.path(dir.path())).expect("reads")["bookmarks"].clone()
}

fn in_environment(setup: &Setup, id: &str) {
    setup
        .core
        .host()
        .update_shell_state(|shell| shell.environment = Some(id.to_owned()));
}

/// The owner's wish (RB-080-48): one bookmark for every environment. The page is on staging
/// while local is in effect - any environment of the project counts.
#[tokio::test(flavor = "multi_thread")]
async fn a_page_of_an_environment_is_stored_as_its_path_and_opens_on_each_environment() {
    let setup = setup();
    let dir = project(&setup, json!([]));
    let core = &setup.core;
    showing(&setup, "https://staging.example.com/shop/cart?x=1#top");
    let answer = run(core, ADD, &json!({ "label": "Cart" })).expect("added");
    assert_eq!(
        answer,
        json!({ "added": "cart", "scope": "project", "relative": true,
                "url": "http://localhost:5173/shop/cart?x=1#top" })
    );
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([{ "id": "cart", "label": "Cart", "path": "/shop/cart?x=1#top" }])
    );
    for (id, url) in [
        ("local", "http://localhost:5173/shop/cart?x=1#top"),
        ("staging", "https://staging.example.com/shop/cart?x=1#top"),
        ("production", "https://www.example.com/shop/cart?x=1#top"),
    ] {
        in_environment(&setup, id);
        assert_eq!(
            run(core, OPEN, &json!({ "id": "cart" })).expect("opens"),
            json!({ "id": "cart", "url": url, "relative": true }),
            "{id}"
        );
        let listed = &summaries(core.host())[0];
        assert_eq!(listed.path.as_deref(), Some("/shop/cart?x=1#top"));
        assert_eq!(listed.url.as_deref(), Some(url), "the tooltip in {id}");
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn a_page_of_a_foreign_host_stays_an_absolute_url() {
    let setup = setup();
    let dir = project(&setup, json!([]));
    let core = &setup.core;
    showing(&setup, "https://docs.example.com/guide?x=1");
    let answer = run(core, ADD, &json!({ "label": "Docs" })).expect("added");
    assert_eq!(answer["relative"], false);
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([{ "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide?x=1" }])
    );
    in_environment(&setup, "production");
    assert_eq!(
        run(core, OPEN, &json!({ "id": "docs" })).expect("opens"),
        json!({ "id": "docs", "url": "https://docs.example.com/guide?x=1", "relative": false })
    );
    assert_eq!(summaries(core.host())[0].path, None);
}

#[tokio::test(flavor = "multi_thread")]
async fn with_follow_environment_off_a_new_bookmark_is_the_whole_url() {
    let setup = setup();
    let dir = project(&setup, json!([]));
    let core = &setup.core;
    let mut user = core.user_settings();
    user.bookmarks.follow_environment = false;
    core.replace_user_settings(user);
    showing(&setup, "http://localhost:5173/shop/cart");
    let answer = run(core, ADD, &json!({ "label": "Cart" })).expect("added");
    assert_eq!(answer["relative"], false);
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([{ "id": "cart", "label": "Cart", "url": "http://localhost:5173/shop/cart" }])
    );
    in_environment(&setup, "staging");
    assert_eq!(
        run(core, OPEN, &json!({ "id": "cart" })).expect("opens")["url"],
        "http://localhost:5173/shop/cart",
        "as before the setting: the host is part of the bookmark"
    );
}

fn last_status(setup: &Setup) -> (String, Option<String>) {
    let message = setup
        .core
        .status_log()
        .recent(1, None)
        .pop()
        .expect("a status message");
    (message.message_key, message.args.get("count").cloned())
}

#[tokio::test(flavor = "multi_thread")]
async fn make_relative_converts_the_bookmarks_of_both_files_and_says_how_many() {
    let setup = setup();
    let dir = project(
        &setup,
        json!([
            { "id": "cart", "label": "Cart", "url": "http://localhost:5173/shop/cart" },
            { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" },
            { "id": "home", "label": "Home", "path": "/" },
            { "id": "pdp", "label": "Product", "url": "https://www.example.com/products/42" }
        ]),
    );
    let local = json!({ "bookmarks": [
        { "id": "mine", "label": "Mine", "url": "https://staging.example.com/me?tab=2" }
    ] });
    rb_config::write_config_file(dir.path(), ConfigFile::Local, &local).expect("written");
    let core = &setup.core;
    assert_eq!(
        run(core, MAKE_RELATIVE, &Value::Null).expect("converted"),
        json!({ "converted": 3, "ids": ["cart", "pdp", "mine"] })
    );
    assert_eq!(
        last_status(&setup),
        (MADE_RELATIVE_KEY.to_owned(), Some("3".to_owned()))
    );
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([
            { "id": "cart", "label": "Cart", "path": "/shop/cart" },
            { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" },
            { "id": "home", "label": "Home", "path": "/" },
            { "id": "pdp", "label": "Product", "path": "/products/42" }
        ])
    );
    assert_eq!(
        stored(&dir, ConfigFile::Local),
        json!([{ "id": "mine", "label": "Mine", "path": "/me?tab=2" }])
    );
    // The list follows at once, and every converted bookmark follows the environment.
    in_environment(&setup, "staging");
    let urls: Vec<Option<String>> = summaries(core.host())
        .into_iter()
        .map(|bookmark| bookmark.url)
        .collect();
    assert_eq!(
        urls,
        [
            Some("https://staging.example.com/shop/cart".to_owned()),
            Some("https://docs.example.com/guide".to_owned()),
            Some("https://staging.example.com/".to_owned()),
            Some("https://staging.example.com/products/42".to_owned()),
            Some("https://staging.example.com/me?tab=2".to_owned()),
        ]
    );
    // Nothing left to convert: no file is written, the status bar says so.
    let files = [ConfigFile::Project, ConfigFile::Local].map(|file| file.path(dir.path()));
    let before = files.clone().map(|path| std::fs::read(path).expect("read"));
    assert_eq!(
        run(core, MAKE_RELATIVE, &Value::Null).expect("runs"),
        json!({ "converted": 0, "ids": [] })
    );
    assert_eq!(
        last_status(&setup),
        (NONE_RELATIVE_KEY.to_owned(), Some("0".to_owned()))
    );
    assert_eq!(files.map(|path| std::fs::read(path).expect("read")), before);
}

/// `bookmarks.make_relative` needs no trust gate (security review RB-090-06, owner question):
/// bookmarks are no protected part, so rewriting both files - the local one with its protected
/// request headers - leaves the trust hash as it was, and a trusted project stays trusted.
#[tokio::test(flavor = "multi_thread")]
async fn make_relative_leaves_the_trust_hash_alone() {
    let setup = setup();
    let dir = project(
        &setup,
        json!([{ "id": "cart", "label": "Cart", "url": "http://localhost:5173/shop/cart" }]),
    );
    let local = json!({
        "emulation": { "headers": { "X-Team": "qa" } },
        "bookmarks": [{ "id": "mine", "label": "Mine", "url": "https://staging.example.com/me" }]
    });
    rb_config::write_config_file(dir.path(), ConfigFile::Local, &local).expect("written");
    let store = rb_config::trust::TrustStore::default();
    let before = rb_config::trust::trust_state(dir.path(), &store).expect("read");
    assert!(!before.hash.is_empty());
    let answer = run(&setup.core, MAKE_RELATIVE, &Value::Null).expect("converted");
    assert_eq!(answer["converted"], 2);
    let after = rb_config::trust::trust_state(dir.path(), &store).expect("read");
    assert_eq!(after.hash, before.hash);
}

#[tokio::test(flavor = "multi_thread")]
async fn make_relative_leaves_a_missing_local_file_missing_and_needs_a_project() {
    let setup = setup();
    assert!(matches!(
        run(&setup.core, MAKE_RELATIVE, &Value::Null),
        Err(rb_commands::CommandError::Failed(_))
    ));
    let dir = project(
        &setup,
        json!([{ "id": "cart", "label": "Cart", "url": "http://localhost:5173/shop/cart" }]),
    );
    assert_eq!(
        run(&setup.core, MAKE_RELATIVE, &Value::Null).expect("converted")["converted"],
        1
    );
    assert!(!ConfigFile::Local.path(dir.path()).exists());
}

/// A path that names another host never reached the config through the loader; one that is in
/// effect all the same (a config the check never saw) is not opened and leads nowhere.
#[tokio::test(flavor = "multi_thread")]
async fn a_path_that_names_another_host_is_not_opened() {
    let setup = setup();
    let host = setup.core.host();
    let mut config = host.config();
    config.environments = vec![environment("local", "http://localhost:5173")];
    config.bookmarks = serde_json::from_value(json!([
        { "id": "evil", "label": "Evil", "path": "//evil.example/login" },
        { "id": "scheme", "label": "Scheme", "path": "https://evil.example/login" }
    ]))
    .expect("bookmarks");
    host.open_config(config, None).expect("opens");
    in_environment(&setup, "local");
    for id in ["evil", "scheme"] {
        assert!(
            matches!(
                run(&setup.core, OPEN, &json!({ "id": id })),
                Err(rb_commands::CommandError::Failed(_))
            ),
            "{id}"
        );
    }
    assert!(
        summaries(host)
            .iter()
            .all(|bookmark| bookmark.url.is_none())
    );
    let navigated = setup.ui.calls().into_iter().any(|call| {
        matches!(call, crate::host::ui::recording::UiCall::Navigate(_, url) if url.contains("evil"))
    });
    assert!(!navigated);
}

/// Review point 2: a project with only a start URL has bookmarks that follow it.
#[tokio::test(flavor = "multi_thread")]
async fn with_only_a_start_url_a_new_bookmark_follows_it() {
    let setup = setup();
    let dir = project_with(&setup, Vec::new(), json!([]));
    showing(&setup, "http://localhost:5173/shop/cart");
    let answer = run(&setup.core, ADD, &json!({ "label": "Cart" })).expect("added");
    assert_eq!(answer["relative"], true);
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([{ "id": "cart", "label": "Cart", "path": "/shop/cart" }])
    );
}

/// Review point 1: under a base with a path, the page beside it keeps its URL.
#[tokio::test(flavor = "multi_thread")]
async fn a_page_beside_the_base_in_effect_keeps_its_url() {
    let setup = setup();
    let dir = project_with(
        &setup,
        vec![environment("local", "http://localhost:5173/app")],
        json!([]),
    );
    let core = &setup.core;
    showing(&setup, "http://localhost:5173/other");
    run(core, ADD, &json!({ "label": "Other" })).expect("added");
    showing(&setup, "http://localhost:5173/app/cart");
    run(core, ADD, &json!({ "label": "Cart" })).expect("added");
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([
            { "id": "other", "label": "Other", "url": "http://localhost:5173/other" },
            { "id": "cart", "label": "Cart", "path": "/cart" }
        ])
    );
    for (id, url) in [
        ("other", "http://localhost:5173/other"),
        ("cart", "http://localhost:5173/app/cart"),
    ] {
        assert_eq!(
            run(core, OPEN, &json!({ "id": id })).expect("opens")["url"],
            url
        );
    }
    // The URL beside the base is no path under the environment that names its host either.
    assert_eq!(
        run(core, MAKE_RELATIVE, &Value::Null).expect("runs")["converted"],
        0
    );
}

/// Review point 2: the dialog sends what was typed and its switch; the core decides.
#[tokio::test(flavor = "multi_thread")]
async fn the_dialogs_switch_stores_a_typed_url_as_a_path_or_says_why_not() {
    let setup = setup();
    let dir = project(&setup, json!([]));
    let core = &setup.core;
    let typed = json!({ "label": "Cart", "url": "https://Staging.Example.com/shop/cart",
                        "follow": true });
    assert_eq!(run(core, ADD, &typed).expect("added")["relative"], true);
    let foreign = json!({ "label": "Docs", "url": "https://docs.example.com/guide",
                          "follow": true });
    assert_eq!(run(core, ADD, &foreign).expect("added")["relative"], false);
    assert_eq!(last_status(&setup).0, KEPT_ABSOLUTE_KEY);
    let whole = json!({ "label": "Home", "path": "/home", "follow": false });
    assert_eq!(run(core, ADD, &whole).expect("added")["relative"], false);
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([
            { "id": "cart", "label": "Cart", "path": "/shop/cart" },
            { "id": "docs", "label": "Docs", "url": "https://docs.example.com/guide" },
            { "id": "home", "label": "Home", "url": "http://localhost:5173/home" }
        ])
    );
}

/// Review point 6: a bookmark in the file that the check refuses (valid before the rule was
/// tightened) is left as it is and named; it blocks neither command.
#[tokio::test(flavor = "multi_thread")]
async fn an_old_invalid_bookmark_is_left_alone_named_and_blocks_nothing() {
    let setup = setup();
    let dir = project(
        &setup,
        json!([{ "id": "cart", "label": "Cart", "url": "http://localhost:5173/shop/cart" }]),
    );
    let path = ConfigFile::Project.path(dir.path());
    let mut document = rb_config::read_json(&path).expect("reads");
    let old = json!({ "id": "cdn", "label": "Cdn", "path": "//cdn.example.com/x" });
    document["bookmarks"]
        .as_array_mut()
        .expect("a list")
        .push(old.clone());
    rb_config::write_config_file(dir.path(), ConfigFile::Project, &document).expect("written");
    let core = &setup.core;
    showing(&setup, "http://localhost:5173/new");
    let added = run(core, ADD, &json!({ "label": "New" })).expect("added");
    assert_eq!(added["invalid"], json!(["cdn"]));
    let status = setup
        .core
        .status_log()
        .recent(1, None)
        .pop()
        .expect("a message");
    assert_eq!(status.message_key, INVALID_KEY);
    assert_eq!(status.args.get("ids").map(String::as_str), Some("cdn"));
    let converted = run(core, MAKE_RELATIVE, &Value::Null).expect("converted");
    assert_eq!(
        converted,
        json!({ "converted": 1, "ids": ["cart"], "invalid": ["cdn"] })
    );
    assert_eq!(
        stored(&dir, ConfigFile::Project),
        json!([
            { "id": "cart", "label": "Cart", "path": "/shop/cart" },
            old,
            { "id": "new", "label": "New", "path": "/new" }
        ])
    );
    // What the command itself would write is still held to the check.
    assert!(
        run(
            core,
            ADD,
            &json!({ "label": "x", "path": "//evil.example" })
        )
        .is_err()
    );
    assert!(run(core, ADD, &json!({ "label": "x", "path": "/a\\b" })).is_err());
}

/// Review point 5: both files or neither. The second write fails: the first file has its old
/// bytes again and a file that was not there is not left behind.
#[test]
fn a_failed_second_write_puts_the_first_file_back() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path();
    let old = json!({ "version": 1, "bookmarks": [{ "id": "a", "label": "A", "url": "http://localhost:5173/a" }] });
    rb_config::write_config_file(root, ConfigFile::Project, &old).expect("written");
    let project_path = ConfigFile::Project.path(root);
    let local_path = ConfigFile::Local.path(root);
    let before = std::fs::read(&project_path).expect("read");
    let new = json!({ "version": 1, "bookmarks": [{ "id": "a", "label": "A", "path": "/a" }] });
    let changed = [(ConfigFile::Project, &new), (ConfigFile::Local, &new)];
    let failing = |file: ConfigFile, document: &Value| match file {
        ConfigFile::Project => {
            rb_config::write_config_file(root, file, document).map_err(|error| error.to_string())
        }
        ConfigFile::Local => {
            // Half written, then the disk is full.
            std::fs::write(&local_path, b"{ \"bookmarks\": [").expect("half");
            Err("no space left".to_owned())
        }
    };
    let error = write_all(root, &changed, &failing).expect_err("fails");
    assert!(error.to_string().contains("config.local.json"), "{error}");
    assert_eq!(std::fs::read(&project_path).expect("read"), before);
    assert!(!local_path.exists(), "the half-written file is gone");
    // With a local file that was there, it gets its bytes back too.
    std::fs::write(&local_path, b"{}").expect("written");
    write_all(root, &changed, &failing).expect_err("fails");
    assert_eq!(std::fs::read(&local_path).expect("read"), b"{}");
    assert_eq!(std::fs::read(&project_path).expect("read"), before);
    // And when nothing fails, both are written.
    let writing = |file: ConfigFile, document: &Value| {
        rb_config::write_config_file(root, file, document).map_err(|error| error.to_string())
    };
    write_all(root, &changed, &writing).expect("written");
    assert_eq!(rb_config::read_json(&project_path).expect("reads"), new);
    assert_eq!(rb_config::read_json(&local_path).expect("reads"), new);
}
