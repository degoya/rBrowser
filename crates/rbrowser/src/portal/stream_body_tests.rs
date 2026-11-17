use super::*;

const PAGE: &str = "http://localhost:3000";
const PORTAL: &str = "http://192.168.1.20:4000";

fn map() -> OriginMap {
    OriginMap::new(PAGE, PORTAL).with("http://localhost:5173", "http://192.168.1.20:4100")
}

/// `body` pushed in the pieces `cuts` makes, then finished: what the device gets.
fn streamed(body: &[u8], cuts: &[usize]) -> Vec<u8> {
    let mut rewrite = StreamRewrite::new(&map());
    let mut out = Vec::new();
    let mut from = 0;
    for &to in cuts.iter().chain(std::iter::once(&body.len())) {
        out.extend(rewrite.push(&body[from..to]));
        assert!(
            rewrite.held() <= map().needles().reach().max(MAX_OPEN_URL),
            "the carry stays small"
        );
        from = to;
    }
    out.extend(rewrite.finish());
    out
}

/// RB-090-72: the page's own origin is rewritten in a streamed body however the body is cut -
/// at every byte of it - plain and escaped, and only whole.
#[test]
fn an_origin_split_anywhere_is_rewritten_as_a_whole_body_would_be() {
    let body = br#"{"img":"http://localhost:3000/media/a.webp","esc":"http:\/\/localhost:3000\/b.webp","no":"http://localhost:30001/","vite":"HTTP://LOCALHOST:5173/x.js","end":"http://localhost:3000"#;
    let whole = map().rewrite_body(body);
    assert!(
        String::from_utf8_lossy(&whole).contains("http://192.168.1.20:4000/media/a.webp"),
        "the reference rewrites"
    );
    for cut in 0..=body.len() {
        assert_eq!(streamed(body, &[cut]), whole, "cut at {cut}");
    }
    let every: Vec<usize> = (1..body.len()).collect();
    assert_eq!(streamed(body, &every), whole, "one byte at a time");
}

/// A stream sends what cannot be the start of an origin at once: a long poll's message is not
/// held back for the next one.
#[test]
fn what_cannot_start_an_origin_goes_out_at_once() {
    let mut rewrite = StreamRewrite::new(&map());
    let out = rewrite.push(b"data: {\"a\":\"http://localhost:3000/x\"}\n\n");
    assert_eq!(out, b"data: {\"a\":\"http://192.168.1.20:4000/x\"}\n\n");
    assert_eq!(rewrite.held(), 0);
    assert_eq!(rewrite.push(b"tail http://loc"), b"tail ");
    assert_eq!(rewrite.held(), "http://loc".len());
    assert_eq!(rewrite.push(b"alhost:3000"), b"");
    assert_eq!(rewrite.push(b"1/"), b"http://localhost:30001/");
    assert_eq!(rewrite.finish(), b"");
}

/// A chunked body is decoded however its bytes arrive; bytes after its end are dropped.
#[test]
fn a_chunked_body_is_decoded_across_any_split() {
    let raw =
        b"5;ext=1\r\nhello\r\n1A\r\n abcdefghijklmnopqrstuvwxy\r\n0\r\nX-Trailer: 1\r\n\r\njunk";
    for cut in 0..=raw.len() {
        let mut reader = BodyReader::new(None, true);
        let mut out = reader.feed(&raw[..cut]).expect("valid");
        out.extend(reader.feed(&raw[cut..]).expect("valid"));
        assert_eq!(out, b"hello abcdefghijklmnopqrstuvwxy", "cut at {cut}");
        assert!(reader.is_done(), "cut at {cut}");
    }
    let mut reader = BodyReader::new(None, true);
    assert_eq!(reader.feed(b"0\r\n\r\n").expect("empty"), b"");
    assert!(reader.is_done());
}

#[test]
fn a_broken_chunked_body_is_refused() {
    for raw in [
        &b"zz\r\nx\r\n"[..],
        b"3\r\nabcX\r\n",
        b"ffffffffffffffffffffffff\r\n",
    ] {
        assert!(BodyReader::new(None, true).feed(raw).is_err(), "{raw:?}");
    }
    let long_line = vec![b'1'; MAX_SIZE_LINE + 1];
    assert!(BodyReader::new(None, true).feed(&long_line).is_err());
    let mut reader = BodyReader::new(None, true);
    reader.feed(b"3\r\nab").expect("so far valid");
    assert!(!reader.complete_at_close(), "cut short");
}

#[test]
fn a_body_of_a_length_ends_there_and_one_without_ends_with_the_connection() {
    let mut reader = BodyReader::new(Some(5), false);
    assert_eq!(reader.feed(b"abc").expect("valid"), b"abc");
    assert!(!reader.is_done() && !reader.complete_at_close());
    assert_eq!(reader.feed(b"defgh").expect("valid"), b"de");
    assert!(reader.is_done());
    let mut reader = BodyReader::new(None, false);
    assert_eq!(reader.feed(b"abc").expect("valid"), b"abc");
    assert!(!reader.is_done() && reader.complete_at_close());
}

#[test]
fn chunks_go_out_in_hex_and_an_empty_one_never_mid_stream() {
    assert_eq!(
        chunk(b"hello world, longer"),
        b"13\r\nhello world, longer\r\n"
    );
    assert_eq!(chunk(b""), b"");
    assert_eq!(LAST_CHUNK, b"0\r\n\r\n");
}

/// The evidence of a streamed body: an unmapped loopback origin, found across a split, once.
#[test]
fn a_streamed_body_finds_an_unmapped_loopback_origin_across_a_split() {
    let mut rewrite = StreamRewrite::new(&map());
    let body = b"{\"a\":\"http://127.0.0.1:9471/a.png\",\"b\":\"http://127.0.0.1:9472/b.png\"}";
    // 20 and 25 cut the first URL's host: it is kept back until it ends.
    for cut in [0, 10, 20, 25, body.len()] {
        let mut rewrite_split = StreamRewrite::new(&map());
        rewrite_split.push(&body[..cut]);
        rewrite_split.push(&body[cut..]);
        rewrite_split.finish();
        assert_eq!(
            rewrite_split.found(),
            [
                "http://127.0.0.1:9471".to_owned(),
                "http://127.0.0.1:9472".to_owned()
            ],
            "cut at {cut}"
        );
    }
    rewrite.push(body);
    rewrite.finish();
    assert_eq!(rewrite.found().len(), 2);
}

/// RB-090-72 review: a read that splits a URL inside its scheme (`http:/` | `/localhost:3000`)
/// keeps the start back, so the scan sees the URL whole - whatever the map's own origins look
/// like (another host, or an `https` one, which share less of the scheme).
#[test]
fn a_url_split_inside_its_scheme_is_still_found() {
    let body =
        b"<img src=\"http://localhost:3000/a.png\"> <img src=\"http:\\/\\/localhost:3001\\/b\">";
    for map in [
        OriginMap::new("http://127.0.0.1:8000", PORTAL),
        OriginMap::new("https://example.test", PORTAL),
    ] {
        for cut in 0..=body.len() {
            let mut rewrite = StreamRewrite::new(&map);
            let mut out = rewrite.push(&body[..cut]);
            out.extend(rewrite.push(&body[cut..]));
            out.extend(rewrite.finish());
            assert_eq!(out, body, "nothing to rewrite, cut at {cut}");
            assert_eq!(
                rewrite.found(),
                [
                    "http://localhost:3000".to_owned(),
                    "http://localhost:3001".to_owned()
                ],
                "{} cut at {cut}",
                map.main_upstream()
            );
        }
    }
}

/// RB-090-75: an origin written as character references (hex upper, hex lower, decimal) is
/// rewritten in a streamed body however the body is cut - at every byte - in its own form,
/// beside the plain one, and only whole.
#[test]
fn an_entity_encoded_origin_split_anywhere_is_rewritten_as_a_whole_body_would_be() {
    let body = b"<img src=\"http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;media&#x2F;a.webp\" \
                 srcset=\"http&#x3a;&#x2f;&#x2f;localhost&#x3a;3000&#x2f;b.webp&#x20;1x,&#x20;\
                 http&#58;&#47;&#47;localhost&#58;3000&#47;c.webp&#32;2x\">\
                 <a href=\"http://localhost:3000/d\">d</a> http&#x3A;&#x2F;&#x2F;localhost&#x3A;30001 \
                 http&#x3A;&#x2F;&#x2F;localhost&#x3A;5173&#x2F;x.js http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000";
    let whole = map().rewrite_body(body);
    let text = String::from_utf8_lossy(&whole);
    for expected in [
        "http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000&#x2F;media",
        "http&#x3a;&#x2f;&#x2f;192.168.1.20&#x3a;4000&#x2f;b.webp",
        "http&#58;&#47;&#47;192.168.1.20&#58;4000&#47;c.webp",
        "http://192.168.1.20:4000/d",
        "localhost&#x3A;30001",
        "http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4100&#x2F;x.js",
    ] {
        assert!(
            text.contains(expected),
            "the reference rewrites {expected}: {text}"
        );
    }
    assert!(
        text.ends_with("http&#x3A;&#x2F;&#x2F;192.168.1.20&#x3A;4000"),
        "{text}"
    );
    for cut in 0..=body.len() {
        assert_eq!(streamed(body, &[cut]), whole, "cut at {cut}");
    }
    let every: Vec<usize> = (1..body.len()).collect();
    assert_eq!(streamed(body, &every), whole, "one byte at a time");
}

/// RB-090-75: an origin without a port is not rewritten while an encoded `:` may still follow
/// it in the next read.
#[test]
fn an_origin_waits_while_a_reference_after_it_is_cut() {
    let map = OriginMap::new("http://localhost", PORTAL);
    let body =
        b"http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;a http&#x3A;&#x2F;&#x2F;localhost&#x2F;b";
    let whole = map.rewrite_body(body);
    for cut in 0..=body.len() {
        let mut rewrite = StreamRewrite::new(&map);
        let mut out = rewrite.push(&body[..cut]);
        out.extend(rewrite.push(&body[cut..]));
        out.extend(rewrite.finish());
        assert_eq!(out, whole, "cut at {cut}");
    }
}

/// RB-090-75: the evidence of a streamed body comes from an encoded URL too, found across every
/// split, once, as the plain origin.
#[test]
fn a_streamed_body_finds_an_entity_encoded_unmapped_loopback_origin_across_any_split() {
    let body = b"<img src=\"http&#x3A;&#x2F;&#x2F;127.0.0.1&#x3A;9471&#x2F;a.png\">\
                 <img srcset=\"http&#58;&#47;&#47;127.0.0.1&#58;9472&#47;b.png&#32;2x\">";
    for cut in 0..=body.len() {
        let mut rewrite = StreamRewrite::new(&map());
        rewrite.push(&body[..cut]);
        rewrite.push(&body[cut..]);
        rewrite.finish();
        assert_eq!(
            rewrite.found(),
            [
                "http://127.0.0.1:9471".to_owned(),
                "http://127.0.0.1:9472".to_owned()
            ],
            "cut at {cut}"
        );
    }
}

/// RB-090-75 review: a mapped origin in a form the rewrite skips is found as a gap in a streamed
/// body too, across every split, once - and not as an unmapped origin.
#[test]
fn a_streamed_body_finds_a_rewrite_gap_across_any_split() {
    let body = b"<img src=\"http&#x3a;&#x2f;&#x2f;localhost&#x003a;3000&#x2f;a.webp\">\
                 <img src=\"http&#x3a;&#x2f;&#x2f;localhost&#x003a;3000&#x2f;b.webp\">";
    for cut in 0..=body.len() {
        let mut rewrite = StreamRewrite::new(&map());
        let mut out = rewrite.push(&body[..cut]);
        out.extend(rewrite.push(&body[cut..]));
        out.extend(rewrite.finish());
        assert_eq!(out, body, "cut at {cut}");
        assert_eq!(
            rewrite.unrewritten(),
            ["http://localhost:3000".to_owned()],
            "cut at {cut}"
        );
        assert!(rewrite.found().is_empty(), "cut at {cut}");
    }
}

/// RB-090-75 review: a long body in many small reads (a slow stream) comes out as the whole
/// body's rewrite, holding back no more than the bound.
#[test]
fn a_body_in_many_small_reads_is_rewritten_as_a_whole() {
    let piece = "<img src=\"http&#x3A;&#x2F;&#x2F;localhost&#x3A;3000&#x2F;a.webp\" \
                 srcset=\"http://localhost:3000/b.webp 1x, http:\\/\\/localhost:5173\\/c 2x\"> h &# \\ ";
    let body = piece.repeat(200).into_bytes();
    let whole = map().rewrite_body(&body);
    for size in [1, 3, 7, 64] {
        let cuts: Vec<usize> = (size..body.len()).step_by(size).collect();
        assert_eq!(streamed(&body, &cuts), whole, "reads of {size} bytes");
    }
}
