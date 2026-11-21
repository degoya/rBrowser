use super::*;

#[test]
fn the_reverse_maps_exactly_one_port_to_the_same_port_and_never_rebinds() {
    assert_eq!(
        reverse_args("R58N123ABC", 3000),
        [
            "-s",
            "R58N123ABC",
            "reverse",
            "--no-rebind",
            "tcp:3000",
            "tcp:3000"
        ]
    );
    assert_eq!(
        unreverse_args("R58N123ABC", 3000),
        ["-s", "R58N123ABC", "reverse", "--remove", "tcp:3000"]
    );
    assert_eq!(
        reverse_list_args("R58N123ABC"),
        ["-s", "R58N123ABC", "reverse", "--list"]
    );
}

/// The narrower rule of RB-096-12 (it replaces RB-095-02's "no call passes reverse"): a
/// `reverse` names only `tcp:<port>` of the same port on both sides - never a range, an
/// address, `-a`, `tcpip` or `--remove-all` - and adds one only with `--no-rebind`.
#[test]
fn a_reverse_call_names_one_loopback_port_and_nothing_else() {
    for port in [80, 443, 3000, 65535] {
        let calls = [
            reverse_args("S1", port),
            unreverse_args("S1", port),
            reverse_list_args("S1"),
        ];
        for call in calls {
            assert_eq!(&call[..3], ["-s", "S1", "reverse"], "{call:?}");
            for banned in ["-a", "tcpip", "--remove-all", "forward"] {
                assert!(!call.iter().any(|arg| arg == banned), "{call:?}");
            }
            let specs: Vec<&String> = call.iter().filter(|a| a.contains(':')).collect();
            for spec in &specs {
                assert_eq!(**spec, format!("tcp:{port}"), "{call:?}");
            }
        }
        let added = reverse_args("S1", port);
        assert!(added.contains(&"--no-rebind".to_owned()), "{added:?}");
        assert_eq!(added[4], added[5], "the device's port is the PC's");
    }
}

#[test]
fn a_loopback_address_names_its_port_or_the_schemes() {
    for (url, port) in [
        ("http://localhost:3000/", 3000),
        ("http://LOCALHOST:5173/app?x=1", 5173),
        ("https://localhost/", 443),
        ("http://127.0.0.1/", 80),
        ("http://127.0.0.1:8080/a", 8080),
        ("http://[::1]:4000/", 4000),
        ("https://[::1]/", 443),
        ("http://user:pw@localhost:9000/", 9000),
    ] {
        assert_eq!(loopback_port(url), Some(port), "{url}");
    }
}

/// Another host is never reversed, nor a scheme that is no page, nor port 0 (which would let
/// adb choose another one).
#[test]
fn another_host_or_scheme_is_no_loopback_port() {
    for url in [
        "https://example.test/",
        "http://192.168.1.20:3000/",
        "http://10.0.2.2:3000/",
        "http://localhost.example.test:3000/",
        "http://127.0.0.2:3000/",
        "http://app.localhost:3000/",
        "ws://localhost:3000/",
        "file:///C:/site/index.html",
        "about:blank",
        "",
        "not a url",
        "http://localhost:0/",
    ] {
        assert_eq!(loopback_port(url), None, "{url}");
    }
}

#[test]
fn the_listed_reverses_are_read_with_or_without_a_transport_name() {
    let listed = parse_reverse_list(
        "UsbFfs tcp:3000 tcp:3000\nhost-19 tcp:8081 tcp:9000\n\n  tcp:5173 tcp:5173  \n",
    );
    assert_eq!(
        listed,
        [
            Listed::new("tcp:3000", "tcp:3000"),
            Listed::new("tcp:8081", "tcp:9000"),
            Listed::new("tcp:5173", "tcp:5173"),
        ]
    );
    assert!(parse_reverse_list("").is_empty());
    assert!(parse_reverse_list("error: closed\n").is_empty());
    assert!(parse_reverse_list("a b c d\n").is_empty(), "an odd line");
}

/// Only a reverse rBrowser set and still sees as `tcp:P tcp:P` is its own; one on the port
/// that someone else set, or that someone pointed elsewhere, is foreign and stays untouched.
#[test]
fn a_port_is_set_kept_or_left_to_its_owner() {
    let listed = parse_reverse_list("x tcp:3000 tcp:3000\nx tcp:8081 tcp:9000\n");
    assert_eq!(decide(3000, &listed, true), Decision::Stands);
    assert_eq!(decide(3000, &listed, false), Decision::Foreign);
    assert_eq!(decide(8081, &listed, true), Decision::Foreign);
    assert_eq!(decide(8081, &listed, false), Decision::Foreign);
    assert_eq!(decide(4000, &listed, false), Decision::Set);
    assert_eq!(
        decide(4000, &listed, true),
        Decision::Set,
        "ours, gone: again"
    );
}

#[test]
fn the_address_a_notice_names_is_host_and_port() {
    assert_eq!(
        address_of("http://localhost:3000/a?b"),
        Some("localhost:3000".to_owned())
    );
    assert_eq!(address_of("https://[::1]/"), Some("[::1]:443".to_owned()));
    assert_eq!(address_of("https://example.test/"), None);
}
