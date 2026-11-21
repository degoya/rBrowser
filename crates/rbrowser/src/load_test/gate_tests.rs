use super::*;

fn gate() -> Gate {
    Gate::new(
        &[
            "https://shop.example.com/start".to_owned(),
            "http://staging.shop.example.com:8080".to_owned(),
            "not a url".to_owned(),
        ],
        &["perf.partner.example".to_owned()],
    )
}

#[test]
fn the_projects_own_origins_and_localhost_need_no_confirmation() {
    let gate = gate();
    assert_eq!(gate.standing("shop.example.com"), Standing::Own);
    assert_eq!(gate.standing("STAGING.shop.example.com."), Standing::Own);
    assert_eq!(gate.standing("localhost"), Standing::Local);
    assert_eq!(gate.standing("app.localhost"), Standing::Local);
    assert_eq!(gate.standing("127.0.0.1"), Standing::Local);
    assert_eq!(gate.standing("127.1.2.3"), Standing::Local);
    assert_eq!(gate.standing("[::1]"), Standing::Local);
}

#[test]
fn any_other_host_only_after_the_person_confirmed_it() {
    let gate = gate();
    assert_eq!(gate.standing("perf.partner.example"), Standing::Confirmed);
    assert_eq!(gate.standing("example.org"), Standing::Unconfirmed);
    // Neither a sibling nor a parent of an own host is one.
    assert_eq!(
        gate.standing("evil-shop.example.com"),
        Standing::Unconfirmed
    );
    assert_eq!(gate.standing("example.com"), Standing::Unconfirmed);
    assert_eq!(gate.standing("10.0.0.5"), Standing::Unconfirmed);
    assert_eq!(gate.standing("[::2]"), Standing::Unconfirmed);
    assert_eq!(
        gate.standing("127.0.0.1.evil.example"),
        Standing::Unconfirmed
    );
    assert_eq!(gate.standing("not a host"), Standing::Unconfirmed);
}

#[test]
fn the_hosts_every_hop_is_held_to() {
    let gate = gate();
    let allowed = gate.allowed(&["127.0.0.7".to_owned(), "example.org".to_owned()]);
    for host in [
        "shop.example.com",
        "staging.shop.example.com",
        "localhost",
        "127.0.0.1",
        "[::1]",
        "perf.partner.example",
        "127.0.0.7",
    ] {
        assert!(allowed.contains(&host.to_owned()), "{host}");
    }
    assert!(!allowed.contains(&"example.org".to_owned()));
}

#[test]
fn the_unconfirmed_targets_are_named() {
    let gate = gate();
    let targets = vec![
        "shop.example.com".to_owned(),
        "example.org".to_owned(),
        "localhost".to_owned(),
    ];
    assert_eq!(gate.unconfirmed(&targets), vec!["example.org".to_owned()]);
}
