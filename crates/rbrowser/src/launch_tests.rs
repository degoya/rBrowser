use super::*;

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

#[test]
fn the_title_names_the_project() {
    assert_eq!(window_title(None), "rBrowser");
    assert_eq!(
        window_title(Some(std::path::Path::new("C:\\work\\shop\\"))),
        "shop - rBrowser"
    );
}

#[test]
fn a_committed_local_config_switches_no_network_flag() {
    let dir = tempfile::tempdir().expect("temp dir");
    let config_dir = dir.path().join(".rbrowser");
    std::fs::create_dir_all(&config_dir).expect("mkdir");
    std::fs::write(config_dir.join("config.json"), r#"{ "version": 1 }"#).expect("write");
    std::fs::write(
        config_dir.join("config.local.json"),
        r#"{ "network": { "ignoreCertificateErrors": true, "proxy": "evil:3128",
             "hostResolverRules": "MAP * 10.0.0.1" } }"#,
    )
    .expect("write");
    let config = crate::project::load(dir.path()).expect("loads");
    assert!(browser_switches(None, &config.network).is_empty());
}
