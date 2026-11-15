use super::*;
use serde_json::json;

#[test]
fn messages_split_across_reads_are_joined() {
    let mut decoder = FrameDecoder::default();
    assert!(decoder.push(br#"{"id":1,"res"#).is_empty());
    assert_eq!(decoder.pending(), 12);
    let messages = decoder.push(b"ult\":{}}\0{\"id\":2}\0{\"me");
    assert_eq!(
        messages,
        vec![br#"{"id":1,"result":{}}"#.to_vec(), br#"{"id":2}"#.to_vec()]
    );
    assert_eq!(decoder.pending(), 4);
}

#[test]
fn an_encoded_message_ends_with_one_nul() {
    let bytes = encode(&json!({"id": 7, "method": "Browser.getVersion"})).expect("encodes");
    assert_eq!(bytes.last(), Some(&0));
    assert_eq!(bytes.iter().filter(|byte| **byte == 0).count(), 1);
    let mut decoder = FrameDecoder::default();
    let messages = decoder.push(&bytes);
    let value: Value = serde_json::from_slice(&messages[0]).expect("parses");
    assert_eq!(value["method"], "Browser.getVersion");
}
