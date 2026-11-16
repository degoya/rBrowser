use super::*;

/// A gzip member of `data` with the flags `flags` and their fields (the CRC is not checked).
pub(crate) fn gzip(data: &[u8], flags: u8) -> Vec<u8> {
    let mut out = vec![0x1f, 0x8b, 8, flags, 0, 0, 0, 0, 0, 3];
    if flags & FEXTRA != 0 {
        out.extend_from_slice(&[3, 0, b'a', b'b', b'c']);
    }
    if flags & FNAME != 0 {
        out.extend_from_slice(b"sitemap.xml\0");
    }
    if flags & FCOMMENT != 0 {
        out.extend_from_slice(b"a comment\0");
    }
    if flags & FHCRC != 0 {
        out.extend_from_slice(&[0, 0]);
    }
    out.extend(miniz_oxide::deflate::compress_to_vec(data, 6));
    out.extend_from_slice(&[0; 8]);
    out
}

#[test]
fn unpacks_a_plain_member() {
    let packed = gzip(b"<urlset/>", 0);
    assert!(is_gzip(&packed));
    assert_eq!(gunzip(&packed, 1024).expect("unpacks"), b"<urlset/>");
}

#[test]
fn skips_every_optional_header_field() {
    let packed = gzip(b"hello", FEXTRA | FNAME | FCOMMENT | FHCRC);
    assert_eq!(gunzip(&packed, 1024).expect("unpacks"), b"hello");
}

#[test]
fn stops_at_the_limit() {
    let bomb = gzip(&vec![b'a'; 1 << 20], 0);
    assert!(bomb.len() < 4096, "a small file");
    assert_eq!(gunzip(&bomb, 64 * 1024), Err(GzipError::TooLarge));
}

#[test]
fn refuses_what_is_no_gzip() {
    assert!(!is_gzip(b"<?xml"));
    assert_eq!(gunzip(b"<?xml version", 1024), Err(GzipError::Malformed));
    assert_eq!(
        gunzip(&[0x1f, 0x8b, 7, 0, 0, 0, 0, 0, 0, 3, 1], 1024),
        Err(GzipError::Malformed)
    );
    let mut cut = gzip(b"x", FNAME);
    cut.truncate(14);
    assert_eq!(gunzip(&cut, 1024), Err(GzipError::Malformed));
}
