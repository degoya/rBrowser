use super::*;

fn reason(failure: &Failure) -> Option<&str> {
    failure
        .params
        .iter()
        .find(|(name, _)| name.as_str() == "reason")
        .map(|(_, value)| value.as_str())
}

#[test]
fn a_full_request_parses() {
    let args = Args::parse(
        r##"{"mode":"invalid","selector":"#register","profile":"testuser","seed":42,
             "strategy":"bounds","target":3,"$console":{}}"##,
    )
    .expect("parses");
    assert_eq!(args.mode, Some(Mode::Invalid));
    assert_eq!(args.selector.as_deref(), Some("#register"));
    assert_eq!(args.profile.as_deref(), Some("testuser"));
    assert_eq!(args.seed, Some(42));
    assert_eq!(args.strategy, Some(Strategy::Bounds));
    assert_eq!(args.target, Target::View(3));
}

#[test]
fn nothing_means_every_view_and_the_settings() {
    for text in ["", "{}", "null", r#"{"target":"all"}"#] {
        let args = Args::parse(text).expect(text);
        assert_eq!(args.target, Target::All);
        assert_eq!(args.mode, None);
        assert_eq!(args.selector, None);
    }
    assert_eq!(
        Args::parse(r#"{"selector":"  "}"#).expect("blank").selector,
        None
    );
}

#[test]
fn bad_arguments_name_their_field() {
    for (text, field) in [
        ("[]", "args"),
        ("nope", "args"),
        (r#"{"mode":"sideways"}"#, "mode"),
        (r#"{"selector":7}"#, "selector"),
        (r#"{"profile":""}"#, "profile"),
        (r#"{"seed":-1}"#, "seed"),
        (r#"{"seed":4294967296}"#, "seed"),
        (r#"{"strategy":"loud"}"#, "strategy"),
        (r#"{"target":"some"}"#, "target"),
        (r#"{"target":-2}"#, "target"),
    ] {
        let failure = Args::parse(text).expect_err(text);
        assert_eq!(failure.code.as_deref(), Some(codes::INVALID), "{text}");
        assert_eq!(reason(&failure), Some(field), "{text}");
    }
    let long = format!(r#"{{"selector":"{}"}}"#, "a".repeat(MAX_SELECTOR + 1));
    assert_eq!(
        reason(&Args::parse(&long).expect_err("long")),
        Some("selector")
    );
}
