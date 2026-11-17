use super::*;

fn token() -> ProxyToken {
    ProxyToken::from_text("a".repeat(64))
}

#[test]
fn a_generated_token_is_64_hex_digits_and_new_each_time() {
    let one = ProxyToken::generate().expect("random");
    let two = ProxyToken::generate().expect("random");
    assert_eq!(one.expose().len(), 64);
    assert_ne!(one, two);
}

#[test]
fn debug_never_shows_the_token() {
    let token = token();
    let shown = format!("{token:?}");
    assert!(!shown.contains(token.expose()));
    assert!(shown.contains("redacted"));
}

#[test]
fn a_matching_bearer_header_authorizes_the_path_as_it_is() {
    let header = format!("Bearer {}", "a".repeat(64));
    let authorized = authorize(&token(), "/json/version", Some(&header)).expect("authorized");
    assert_eq!(authorized.path, "/json/version");
    assert_eq!(authorized.proof, Proof::Header);
}

#[test]
fn the_bearer_scheme_is_case_insensitive() {
    let header = format!("bearer   {}", "a".repeat(64));
    assert!(authorize(&token(), "/json", Some(&header)).is_some());
}

#[test]
fn a_path_token_is_cut_from_the_path() {
    let path = format!("/t/{}/devtools/browser/x", "a".repeat(64));
    let authorized = authorize(&token(), &path, None).expect("authorized");
    assert_eq!(authorized.path, "/devtools/browser/x");
    assert_eq!(authorized.proof, Proof::Path);
    let bare = format!("/t/{}", "a".repeat(64));
    assert_eq!(authorize(&token(), &bare, None).expect("bare").path, "/");
}

#[test]
fn no_token_a_wrong_one_or_a_wrong_scheme_is_refused() {
    assert_eq!(authorize(&token(), "/json/version", None), None);
    let wrong = format!("Bearer {}", "b".repeat(64));
    assert_eq!(authorize(&token(), "/json", Some(&wrong)), None);
    let basic = format!("Basic {}", "a".repeat(64));
    assert_eq!(authorize(&token(), "/json", Some(&basic)), None);
    assert_eq!(authorize(&token(), "/t/short/json", None), None);
    assert_eq!(authorize(&token(), "/t//json", None), None);
}

#[test]
fn every_proof_offered_must_match() {
    let good = format!("Bearer {}", "a".repeat(64));
    let wrong_path = format!("/t/{}/json", "b".repeat(64));
    assert_eq!(authorize(&token(), &wrong_path, Some(&good)), None);
    let good_path = format!("/t/{}/json", "a".repeat(64));
    let wrong = format!("Bearer {}", "b".repeat(64));
    assert_eq!(authorize(&token(), &good_path, Some(&wrong)), None);
    let both = authorize(&token(), &good_path, Some(&good)).expect("both");
    assert_eq!(both.proof, Proof::Path);
}

#[test]
fn the_path_prefix_carries_the_token_for_answers() {
    let token = token();
    assert_eq!(token.path_prefix(), format!("/t/{}", "a".repeat(64)));
    assert_eq!(token.bearer(), format!("Bearer {}", "a".repeat(64)));
}
