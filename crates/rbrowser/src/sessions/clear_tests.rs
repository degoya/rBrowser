use super::*;

#[test]
fn every_origin_once_in_order() {
    let urls = [
        "https://a.test/1",
        "about:blank",
        "https://b.test/",
        "https://a.test/2",
    ];
    assert_eq!(origins_to_clear(urls), ["https://a.test", "https://b.test"]);
}
