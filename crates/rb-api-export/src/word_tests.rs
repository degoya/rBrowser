use super::*;

#[test]
fn environment_style_turns_variables_into_env_pieces_and_lists_them_once() {
    let mut words = Words::new(Variables::Environment);
    let word = words.of("Bearer {{token}} {{$guid}} {{token}}");
    assert_eq!(
        word.segs,
        [
            Seg::Text("Bearer ".into()),
            Seg::Env("token".into()),
            Seg::Text(" {{$guid}} ".into()),
            Seg::Env("token".into()),
        ]
    );
    assert_eq!(words.into_env(), ["token"]);
}

#[test]
fn template_style_keeps_every_variable_as_text() {
    let mut words = Words::new(Variables::Template);
    let word = words.of("Bearer {{token}}");
    assert_eq!(word.text_only().as_deref(), Some("Bearer {{token}}"));
    assert!(words.into_env().is_empty());
}

#[test]
fn words_join_their_texts() {
    let mut word = Word::lit("a");
    word.push_text("b");
    word.push_text("");
    assert_eq!(word.segs, [Seg::Text("ab".into())]);
    assert!(word.has_any(&['b']));
    assert_eq!(Word::default().text_only().as_deref(), Some(""));
}
