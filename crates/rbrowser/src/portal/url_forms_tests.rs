use super::*;

/// RB-090-75: an attribute escaper writes every byte but letters, digits and `,.-_` as a
/// character reference; the dots of an IP address and a host stay literal.
#[test]
fn each_form_writes_the_punctuation_of_an_origin() {
    let origin = "http://192.168.1.20:4000";
    assert_eq!(Form::Plain.encode(origin), b"http://192.168.1.20:4000");
    assert_eq!(Form::Escaped.encode(origin), br"http:\/\/192.168.1.20:4000");
    assert_eq!(
        Form::Hex { upper: true }.encode(origin),
        b"http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000"
    );
    assert_eq!(
        Form::Hex { upper: false }.encode(origin),
        b"http&#x3a;&#x2f;&#x2f;192.168.1.20&#x3a;4000"
    );
    assert_eq!(
        Form::Decimal.encode(origin),
        b"http&#58;&#47;&#47;192.168.1.20&#58;4000"
    );
    assert_eq!(
        Form::Hex { upper: true }.encode("http://[::1]:5173"),
        b"http&#x3A;&#x2F;&#x2F;&#x5B;&#x3A;&#x3A;1&#x5D;&#x3A;5173"
    );
    assert_eq!(
        Form::Decimal.encode("http://my-host_1.test"),
        b"http&#58;&#47;&#47;my-host_1.test"
    );
}

#[test]
fn a_scheme_is_known_in_every_form_and_its_start_may_still_become_one() {
    for scheme in [
        &b"http://"[..],
        br"http:\/\/",
        b"http&#x3a;&#x2f;&#x2f;",
        b"http&#58;&#47;&#47;",
    ] {
        let mut text = scheme.to_vec();
        text.extend_from_slice(b"localhost");
        assert_eq!(scheme_at(&text, 0), Some(scheme.len()), "{text:?}");
        for end in 1..scheme.len() {
            assert!(could_be_scheme(&scheme[..end]), "{:?}", &scheme[..end]);
        }
        assert!(
            !could_be_scheme(scheme),
            "a whole scheme is no longer a start"
        );
    }
    assert_eq!(scheme_at(b"http:/x", 0), None);
    assert_eq!(scheme_at(b"http&#x3a;x", 0), None);
    assert!(!could_be_scheme(b"https"));
    assert!(!could_be_scheme(b""));
}

#[test]
fn a_character_reference_reads_as_its_ascii_byte() {
    assert_eq!(char_at(b"&#x3a;1", 0), Some((b':', 6)));
    assert_eq!(char_at(b"&#58;1", 0), Some((b':', 5)));
    assert_eq!(char_at(b"&#x00003a;", 0), Some((b':', 10)));
    assert_eq!(char_at(b"a", 0), Some((b'a', 1)));
    assert_eq!(char_at(b"a", 1), None);
    // Not a numeric reference, or not ASCII: the `&` stands for itself.
    for text in [
        &b"&amp;"[..],
        b"&#x;",
        b"&#x3a",
        b"&#xe9;",
        b"&#12345678;",
        b"&#x1234567;",
    ] {
        assert_eq!(char_at(text, 0), Some((b'&', 1)), "{text:?}");
    }
}

#[test]
fn the_start_of_a_reference_is_told_from_text() {
    for rest in [&b"&"[..], b"&#", b"&#x", b"&#x3", b"&#5", b"&#x00003"] {
        assert!(partial_reference(rest), "{rest:?}");
    }
    for rest in [&b"&#x3a;"[..], b"&a", b"&#xg", b"&#x0000003", b"x", b""] {
        assert!(!partial_reference(rest), "{rest:?}");
    }
}

/// An origin ends where no host or port goes on - written plain or as a reference.
#[test]
fn an_origin_is_whole_only_where_neither_a_byte_nor_a_reference_goes_on() {
    let cases: [(&[u8], bool); 12] = [
        (b"x", true),
        (b"x/", true),
        (b"x&#x2f;", true),
        (b"x&#47;", true),
        (b"x\"", true),
        (b"x&amp;", true),
        (b"x1", false),
        (b"x:", false),
        (b"x.", false),
        (b"x&#x3a;3000", false),
        (b"x&#58;3000", false),
        (b"x&#x2e;example", false),
    ];
    for (text, whole) in cases {
        assert_eq!(is_whole(text, 1), whole, "{text:?}");
    }
}

/// The host and port of a URL, decoded; open when they may still go on past the end.
#[test]
fn an_authority_is_read_through_its_references() {
    let read = authority_at(b"localhost&#x3a;3000&#x2f;media", 0);
    assert_eq!(
        (read.text.as_str(), read.end, read.open),
        ("localhost:3000", 19, false)
    );
    let read = authority_at(b"&#x5b;&#x3a;&#x3a;1&#x5d;&#58;5173\"", 0);
    assert_eq!((read.text.as_str(), read.open), ("[::1]:5173", false));
    let read = authority_at(b"127.0.0.1:9000/a", 0);
    assert_eq!(
        (read.text.as_str(), read.end, read.open),
        ("127.0.0.1:9000", 14, false)
    );
    for text in [&b"localhost"[..], b"localhost&#x3", b"localhost&"] {
        let read = authority_at(text, 0);
        assert!(read.open, "{text:?}");
        assert_eq!(read.end, text.len());
    }
}

#[test]
fn the_case_of_a_hex_reference_is_read_from_the_first_one() {
    assert!(upper_hex(b"http&#x3A;&#x2F;&#x2F;a"));
    assert!(upper_hex(b"http&#X3a;"));
    assert!(!upper_hex(b"http&#x3a;&#x2f;&#x2f;A"));
    assert!(!upper_hex(b"http://A"));
}
