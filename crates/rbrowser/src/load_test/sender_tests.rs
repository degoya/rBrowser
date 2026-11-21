use super::*;

fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
    list.iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn every_request_names_the_load_test_and_hides_no_one() {
    let sent = identified(&pairs(&[
        ("Accept", "application/json"),
        ("user-agent", "Mozilla/5.0"),
        ("X-Forwarded-For", "10.1.2.3"),
        ("Forwarded", "for=10.1.2.3"),
        ("X-Real-IP", "10.1.2.3"),
        ("Authorization", "Bearer x"),
    ]));
    assert_eq!(
        sent,
        pairs(&[
            ("Accept", "application/json"),
            ("Authorization", "Bearer x"),
            ("User-Agent", USER_AGENT),
        ])
    );
    assert!(USER_AGENT.contains("rBrowser load test"));
}

#[test]
fn a_failure_without_an_answer_has_its_kind() {
    let refused = ureq::Error::Io(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "refused",
    ));
    assert_eq!(kind_of(&refused), "connection");
    assert_eq!(kind_of(&ureq::Error::HostNotFound), "connection");
    let timed_out = ureq::Error::Io(std::io::Error::new(std::io::ErrorKind::TimedOut, "slow"));
    assert_eq!(kind_of(&timed_out), "timeout");
    assert_eq!(kind_of(&ureq::Error::BadUri("x".into())), "other");
}
