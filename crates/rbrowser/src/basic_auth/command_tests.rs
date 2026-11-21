use serde_json::json;

use super::*;

#[test]
fn without_arguments_the_shell_asks() {
    assert!(matches!(parse(&Value::Null), Ok(Action::Open)));
    assert!(matches!(parse(&json!({})), Ok(Action::Open)));
}

#[test]
fn a_user_and_password_set_the_credential() {
    let args = json!({ "environment": "staging", "user": "ada", "password": "pw-1234" });
    match parse(&args) {
        Ok(Action::Set {
            environment,
            credential,
        }) => {
            assert_eq!(environment, "staging");
            assert_eq!(credential.user(), "ada");
            assert_eq!(credential.password(), "pw-1234");
        }
        _ => panic!("a set"),
    }
}

#[test]
fn remove_takes_no_credential() {
    assert!(matches!(
        parse(&json!({ "environment": "staging", "remove": true })),
        Ok(Action::Remove { environment }) if environment == "staging"
    ));
    assert!(
        parse(&json!({ "environment": "staging", "remove": true, "password": "x-1234" })).is_err()
    );
}

#[test]
fn no_error_ever_repeats_what_was_sent() {
    // serde names the value it could not take ("invalid type: string \"...\""); the command's
    // error is fixed text, so a password typed into the wrong field reaches no notice or log.
    let secret = "the-secret-password-42";
    for args in [
        json!({ "environment": "staging", "remove": secret }),
        json!({ "environment": "staging", "user": "ada", "password": 42, "x": secret }),
        json!({ "environment": secret, "user": "ada:x", "password": secret }),
        json!({ "environment": "staging", "user": "ada", "password": format!("{secret}\n") }),
        json!({ "environment": "staging", "user": "ada" }),
        json!({ secret: true }),
        json!(secret),
    ] {
        let error = match parse(&args) {
            Err(error) => error.to_string(),
            Ok(_) => panic!("{args} must be refused"),
        };
        assert!(!error.contains(secret), "{error}");
        assert!(!error.contains("the-secret"), "{error}");
    }
}

#[test]
fn an_environment_id_that_is_no_plain_name_is_refused() {
    let long = "e".repeat(129);
    for id in ["", "a/b", "x\ny", long.as_str()] {
        assert!(
            parse(&json!({ "environment": id, "remove": true })).is_err(),
            "{id:?}"
        );
    }
}
