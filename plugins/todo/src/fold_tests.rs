use super::{FOLD, fold};

#[test]
fn case_and_accents_go_composed_or_not() {
    assert_eq!(fold("Cr\u{e8}me BR\u{db}L\u{c9}E"), "creme brulee");
    assert_eq!(fold("Cre\u{300}me"), "creme");
    assert_eq!(fold("\u{c4}rger Stra\u{df}e"), "arger stra\u{df}e");
    assert_eq!(
        fold("\u{386}\u{3bb}\u{3c6}\u{3b1}"),
        "\u{3b1}\u{3bb}\u{3c6}\u{3b1}"
    );
    assert_eq!(fold("\u{130}stanbul"), "istanbul");
    assert_eq!(fold("Plain ASCII"), "plain ascii");
}

#[test]
fn the_table_is_sorted_for_its_search() {
    assert!(FOLD.windows(2).all(|pair| pair[0].0 < pair[1].0));
}
