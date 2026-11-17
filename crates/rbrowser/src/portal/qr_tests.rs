use super::*;

#[test]
fn a_pairing_url_becomes_a_square_matrix_with_finder_patterns() {
    let url = format!(
        "http://192.168.178.120:54321/__rbrowser/pair?t={}",
        "a".repeat(32)
    );
    let matrix = QrMatrix::encode(&url).expect("fits");
    assert_eq!(matrix.rows.len(), matrix.size);
    assert!(matrix.rows.iter().all(|row| row.len() == matrix.size));
    assert!(
        matrix
            .rows
            .iter()
            .all(|row| row.bytes().all(|b| b == b'0' || b == b'1'))
    );
    // The top-left finder pattern: a dark 7x7 ring.
    assert_eq!(&matrix.rows[0][..7], "1111111");
    assert_eq!(&matrix.rows[1][..7], "1000001");
    assert_eq!(&matrix.rows[6][..7], "1111111");
}

#[test]
fn the_matrix_hides_its_content_in_debug() {
    let matrix = QrMatrix::encode("http://x/__rbrowser/pair?t=secret").expect("fits");
    let debug = format!("{matrix:?}");
    assert!(!debug.contains("1111111"), "{debug}");
}
