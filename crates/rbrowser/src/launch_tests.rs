use super::*;
use crate::host::ui::recording::UiCall;

#[test]
fn the_pipe_comes_first_and_no_port_is_ever_asked_for() {
    let switches = browser_switches(Some("1234,5678"), &NetworkConfig::default());
    assert_eq!(
        switches,
        vec![
            ("remote-debugging-pipe".to_owned(), None),
            (
                "remote-debugging-io-pipes".to_owned(),
                Some("1234,5678".to_owned())
            ),
        ]
    );
    assert!(
        !switches
            .iter()
            .any(|(name, _)| name == "remote-debugging-port")
    );
    assert!(browser_switches(None, &NetworkConfig::default()).is_empty());
}

#[test]
fn the_project_network_settings_become_switches() {
    let network = NetworkConfig {
        host_resolver_rules: Some("MAP shop.test 127.0.0.1".to_owned()),
        ignore_certificate_errors: true,
        proxy: Some(String::new()),
    };
    let switches = browser_switches(None, &network);
    assert_eq!(
        switches,
        vec![
            (
                "host-resolver-rules".to_owned(),
                Some("MAP shop.test 127.0.0.1".to_owned())
            ),
            ("ignore-certificate-errors".to_owned(), None),
        ]
    );
}

#[test]
fn a_pac_url_is_not_a_proxy_server() {
    let mut network = NetworkConfig {
        proxy: Some("http://corp/proxy.PAC?v=2".to_owned()),
        ..NetworkConfig::default()
    };
    assert_eq!(browser_switches(None, &network)[0].0, "proxy-pac-url");
    network.proxy = Some("http://proxy:8080".to_owned());
    assert_eq!(browser_switches(None, &network)[0].0, "proxy-server");
}

/// The title bar's two switches (RB-094-19).
fn bar(show_version: bool, show_credit: bool) -> TitleBarSettings {
    TitleBarSettings {
        show_version,
        show_credit,
    }
}

#[test]
fn the_title_names_the_project() {
    let plain = bar(false, false);
    assert_eq!(window_title(None, &plain), "rBrowser");
    assert_eq!(
        window_title(Some(std::path::Path::new("C:\\work\\shop\\")), &plain),
        "shop - rBrowser"
    );
}

// RB-094-19: the version and the credit line, each by its setting, with and without a project.
#[test]
fn the_title_shows_the_version_and_the_credit_by_their_settings() {
    let version = env!("CARGO_PKG_VERSION");
    let credit = "made with Love by Alexander Herling/DEGOYA.digital";
    assert_eq!(CREDIT, credit);
    let shop = Some(std::path::Path::new("/home/dev/shop"));
    let cases = [
        (bar(true, true), format!("rBrowser {version} - {credit}")),
        (bar(true, false), format!("rBrowser {version}")),
        (bar(false, true), format!("rBrowser - {credit}")),
        (bar(false, false), "rBrowser".to_owned()),
    ];
    for (settings, product) in cases {
        assert_eq!(window_title(None, &settings), product, "{settings:?}");
        assert_eq!(
            window_title(shop, &settings),
            format!("shop - {product}"),
            "{settings:?}"
        );
    }
    assert_eq!(
        window_title(shop, &TitleBarSettings::default()),
        format!("shop - rBrowser {version} - {credit}"),
        "both are on by default"
    );
}

// RB-094-19: a changed setting reaches the main window at once, with the open project's name.
#[tokio::test]
async fn the_main_window_follows_the_title_bar_settings_at_once() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().join("shop");
    std::fs::create_dir_all(&root).expect("mkdir");
    let setup =
        crate::layout::test_support::setup_in(crate::layout::test_support::config(), Some(root));
    let titles = || {
        setup
            .ui
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                UiCall::WindowTitle(title) => Some(title),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let version = env!("CARGO_PKG_VERSION");
    let mut user = setup.core.user_settings();
    user.ui.title_bar.show_credit = false;
    setup.core.replace_user_settings(user);
    assert_eq!(titles().last(), Some(&format!("shop - rBrowser {version}")));
    let mut user = setup.core.user_settings();
    user.ui.title_bar.show_version = false;
    setup.core.replace_user_settings(user);
    assert_eq!(titles().last(), Some(&"shop - rBrowser".to_owned()));
    let mut user = setup.core.user_settings();
    user.ui.title_bar = TitleBarSettings::default();
    setup.core.replace_user_settings(user);
    assert_eq!(
        titles().last(),
        Some(&format!("shop - rBrowser {version} - {CREDIT}"))
    );
}
