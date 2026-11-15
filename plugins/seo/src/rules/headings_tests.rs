use crate::testing::{check, serve, short};

const URL: &str = "https://shop.test/";

fn headings(body: &str) -> Vec<String> {
    let mut host = serve(URL, &format!("<html><body>{body}</body></html>"), &[]);
    short(&check(&mut host, URL, None), "headings")
}

#[test]
fn one_h1_and_steps_of_one_are_quiet() {
    assert!(headings("<h1>A</h1><h2>B</h2><h3>C</h3><h2>D</h2><h2>E</h2>").is_empty());
}

#[test]
fn a_missing_h1_a_second_one_and_a_skipped_level_are_reported() {
    assert_eq!(
        headings("<h2>A</h2>"),
        ["headings.h1_missing headings.h1_missing"]
    );
    assert_eq!(
        headings("<h1>A</h1><h2>B</h2><h4>C</h4><h1>D</h1>"),
        [
            "headings.h1_multiple headings.h1_multiple",
            "headings.skipped headings.skipped"
        ]
    );
}
