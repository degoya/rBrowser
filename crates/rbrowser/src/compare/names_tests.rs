use rb_visual::slug::valid_name;

use super::*;

#[test]
fn the_names_are_plain_short_and_numbered() {
    assert_eq!(path_folder(0, "/shop/cart?page=2"), "01-shop-cart-page-2");
    assert_eq!(path_folder(9, "/"), "10-index");
    assert_eq!(shot_name(1, "staging", 0, "mobile"), "2-staging--1-mobile");
    assert_eq!(diff_name(0, 2, 3, "desktop"), "diff-1-3--4-desktop");
    let long = "a".repeat(200);
    for name in [
        path_folder(98, &format!("/{long}")),
        shot_name(7, &format!("{long}/..x"), 63, &long),
        diff_name(6, 7, 63, &long),
        shot_name(0, "...", 0, "..."),
    ] {
        assert!(valid_name(&name), "{name}");
    }
}
