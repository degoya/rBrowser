use super::McpToken;

#[test]
fn generate_gives_64_hex_digits_each_time_new() {
    let a = McpToken::generate().expect("the OS gives randomness");
    let b = McpToken::generate().expect("the OS gives randomness");
    assert_eq!(a.expose().len(), 64);
    assert_ne!(a, b);
}

#[test]
fn debug_never_shows_the_token() {
    let token = McpToken::from_text("0123456789abcdef0123456789abcdef");
    let shown = format!("{token:?}");
    assert!(!shown.contains("0123456789abcdef"), "{shown}");
    assert!(shown.contains("redacted"));
}

#[test]
fn matches_only_the_same_text() {
    let token = McpToken::from_text("abcd1234");
    assert!(token.matches("abcd1234"));
    assert!(!token.matches("abcd1235"));
    assert!(!token.matches("abcd123"));
    assert!(!token.matches(""));
}

#[test]
fn proof_depends_on_token_port_and_nonce() {
    let token = McpToken::from_text("a".repeat(64));
    let other = McpToken::from_text("b".repeat(64));
    let nonce = "0".repeat(32);
    let proof = token.proof(4000, &nonce);
    assert_eq!(proof.len(), 64);
    assert_eq!(proof, token.proof(4000, &nonce));
    assert_ne!(proof, token.proof(4001, &nonce));
    assert_ne!(proof, token.proof(4000, &"1".repeat(32)));
    assert_ne!(proof, other.proof(4000, &nonce));
    assert!(!proof.contains(token.expose()));
}

#[test]
fn proof_check_compares_whole_proofs() {
    let token = McpToken::from_text("c".repeat(64));
    let nonce = "f".repeat(40);
    let proof = token.proof(1234, &nonce);
    assert!(token.proof_matches(1234, &nonce, &proof));
    assert!(!token.proof_matches(1234, &nonce, &proof[..63]));
    assert!(!token.proof_matches(1235, &nonce, &proof));
}
