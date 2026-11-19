use super::*;
use crate::event::Offset;

fn target(element: ElementSelector) -> ElementTarget {
    ElementTarget {
        hosts: Vec::new(),
        element,
        offset: Offset { x: 0.5, y: 0.5 },
    }
}

fn css(path: &str) -> ElementTarget {
    target(ElementSelector::Css { path: path.into() })
}

#[test]
fn a_password_field_is_secret_whatever_the_settings() {
    assert!(is_secret(true, &css("#name"), &[]));
    assert!(!is_secret(false, &css("#name"), &[]));
}

#[test]
fn extra_entries_match_the_path_the_testid_or_the_name() {
    let extra = |entry: &str| vec![entry.to_owned()];
    assert!(is_secret(false, &css("#otp"), &extra("#otp")));
    assert!(is_secret(
        false,
        &css("form#login > input#otp"),
        &extra("input#otp")
    ));
    assert!(!is_secret(false, &css("#otp-hint"), &extra("#otp")));
    let testid = target(ElementSelector::Testid {
        testid: "pin".into(),
        nth: 0,
        path: "#f > input".into(),
    });
    assert!(is_secret(false, &testid, &extra("[data-testid=\"pin\"]")));
    let role = target(ElementSelector::Role {
        role: "textbox".into(),
        name: "One-time code".into(),
        nth: 0,
        visible_nth: None,
        within: None,
        path: "#f > input".into(),
    });
    assert!(is_secret(false, &role, &extra("one-time CODE")));
    assert!(!is_secret(false, &role, &extra("  ")));
}

#[test]
fn the_name_comes_from_the_field() {
    assert_eq!(secret_name(&css("#password")), "password");
    assert_eq!(secret_name(&css("form#login > input#pw.big")), "pw");
    assert_eq!(secret_name(&css("form > input:nth-of-type(2)")), "secret");
    let testid = target(ElementSelector::Testid {
        testid: "Login Password".into(),
        nth: 0,
        path: "#x".into(),
    });
    assert_eq!(secret_name(&testid), "login-password");
    let role = target(ElementSelector::Role {
        role: "textbox".into(),
        name: "One-time code!".into(),
        nth: 0,
        visible_nth: None,
        within: None,
        path: "#x".into(),
    });
    assert_eq!(secret_name(&role), "one-time-code");
}

#[test]
fn a_name_never_breaks_the_placeholder() {
    let odd = target(ElementSelector::Testid {
        testid: "}}{{env.X}}".into(),
        nth: 0,
        path: "#x".into(),
    });
    let name = secret_name(&odd);
    assert!(
        name.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
        "{name}"
    );
    let long = target(ElementSelector::Testid {
        testid: "a".repeat(100),
        nth: 0,
        path: "#x".into(),
    });
    assert_eq!(secret_name(&long).len(), 32);
}

#[test]
fn a_url_s_known_tokens_are_masked_in_the_query_and_the_fragment() {
    assert_eq!(
        mask_url(
            "https://app.test/cb?code=4%2Fabc&state=xyz&SIG=1;token=t#id_token=e.y.j&scope=a",
            &[]
        ),
        "https://app.test/cb?code=********&state=xyz&SIG=********;token=********\
         #id_token=********&scope=a"
    );
    // A hash route's own query too.
    assert_eq!(
        mask_url("https://app.test/#/done?access_token=abc&x=1", &[]),
        "https://app.test/#/done?access_token=********&x=1"
    );
    assert_eq!(
        mask_url("https://s3.test/f?X-Amz-Signature=ab12&X-Amz-Date=1", &[]),
        "https://s3.test/f?X-Amz-Signature=********&X-Amz-Date=1"
    );
}

#[test]
fn a_url_without_tokens_stays_as_it_is() {
    for url in [
        "https://app.test/",
        "https://app.test/code/token?q=code&page=2#code",
        "https://app.test/?code",
        "https://app.test/?=x&&",
        "about:blank",
    ] {
        assert_eq!(mask_url(url, &[]), url);
    }
}

#[test]
fn a_secret_field_entry_also_names_a_url_parameter() {
    let extra = vec![" Session ".to_owned(), "#otp".to_owned()];
    assert_eq!(
        mask_url("https://app.test/?session=s3cr3t&otp=1", &extra),
        "https://app.test/?session=********&otp=1"
    );
}
