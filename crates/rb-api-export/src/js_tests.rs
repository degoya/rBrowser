use super::*;

#[test]
fn plain_text_is_a_single_quoted_string() {
    assert_eq!(lit("it's"), "'it\\'s'");
    assert_eq!(lit("a\nb\\c\u{1}"), "'a\\nb\\\\c\\u0001'");
    assert_eq!(lit("{{token}}"), "'{{token}}'");
}

#[test]
fn an_environment_variable_makes_a_template_literal() {
    let word = Word {
        segs: vec![Seg::Text("Bearer `$ ".into()), Seg::Env("token".into())],
    };
    assert_eq!(string(&word), "`Bearer \\`\\$ ${process.env.token}`");
}

#[test]
fn objects_and_repeats() {
    assert_eq!(object(&[], 2), "{}");
    assert_eq!(
        object(&[("'a'".into(), "'1'".into())], 2),
        "{\n    'a': '1',\n  }"
    );
    assert!(repeats(["Accept", "accept"]));
    assert!(!repeats(["a", "b"]));
}
