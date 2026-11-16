use super::*;

#[test]
fn length_delimited_fields_are_read_in_order() {
    // field 1 "ab", field 10000 "c" (tag 80002 = 0x82 0xf1 0x04).
    let bytes = [0x0a, 2, b'a', b'b', 0x82, 0xf1, 0x04, 1, b'c'];
    let fields = fields(&bytes).expect("parses");
    assert_eq!(fields.len(), 2);
    assert_eq!((fields[0].number, fields[0].bytes), (1, &b"ab"[..]));
    assert_eq!((fields[1].number, fields[1].bytes), (10_000, &b"c"[..]));
}

#[test]
fn varint_and_fixed_fields_are_skipped() {
    // field 4 varint 300, field 5 fixed64, field 6 fixed32, then field 1 "x".
    let mut bytes = vec![0x20, 0xac, 0x02, 0x29];
    bytes.extend_from_slice(&[0; 8]);
    bytes.push(0x35);
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&[0x0a, 1, b'x']);
    let fields = fields(&bytes).expect("parses");
    assert_eq!(fields.len(), 1);
    assert_eq!((fields[0].number, fields[0].bytes), (1, &b"x"[..]));
}

#[test]
fn a_length_past_the_end_is_truncated() {
    assert_eq!(
        fields(&[0x0a, 5, b'a']).expect_err("refused"),
        ProtoError::Truncated
    );
    assert_eq!(fields(&[0x0a]).expect_err("refused"), ProtoError::Truncated);
    assert_eq!(fields(&[0x20]).expect_err("refused"), ProtoError::Truncated);
}

#[test]
fn groups_and_unknown_wire_types_are_refused() {
    assert_eq!(
        fields(&[0x0b]).expect_err("refused"),
        ProtoError::BadWireType(3)
    );
    assert_eq!(
        fields(&[0x0e]).expect_err("refused"),
        ProtoError::BadWireType(6)
    );
}

#[test]
fn a_varint_longer_than_ten_bytes_is_refused() {
    let bytes = [0x80; 11];
    assert_eq!(fields(&bytes).expect_err("refused"), ProtoError::BadVarint);
}

#[test]
fn an_empty_message_has_no_fields() {
    assert!(fields(&[]).expect("parses").is_empty());
}
