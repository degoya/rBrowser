use super::*;
use crate::word::{Seg, Word};

fn bearer() -> Word {
    Word {
        segs: vec![
            Seg::Text("Authorization: Bearer ".into()),
            Seg::Env("token".into()),
        ],
    }
}

fn quoted(shell: Shell, word: &Word) -> String {
    Renderer::new(shell, false).quote(word)
}

#[test]
fn posix_leaves_plain_words_bare_and_single_quotes_the_rest() {
    let q = |text: &str| quoted(Shell::Posix, &Word::lit(text));
    assert_eq!(q("--header"), "--header");
    assert_eq!(
        q("https://api.example.com/users"),
        "https://api.example.com/users"
    );
    assert_eq!(q("https://x/a?b=1&c=2"), "'https://x/a?b=1&c=2'");
    assert_eq!(q("it's"), "'it'\\''s'");
    assert_eq!(q(""), "''");
    assert_eq!(q("{{token}}"), "'{{token}}'");
    assert_eq!(
        quoted(Shell::Posix, &bearer()),
        "'Authorization: Bearer '\"${token}\""
    );
}

#[test]
fn powershell_doubles_its_quotes_and_reads_env_inside_double_quotes() {
    let q = |text: &str| quoted(Shell::PowerShell, &Word::lit(text));
    assert_eq!(q("--header"), "--header");
    assert_eq!(q("@./logo.png"), "'@./logo.png'");
    assert_eq!(q("it's"), "'it''s'");
    assert_eq!(q("a\u{2019}b"), "'a\u{2019}\u{2019}b'");
    assert_eq!(q("a,b"), "'a,b'");
    assert_eq!(
        quoted(Shell::PowerShell, &bearer()),
        "\"Authorization: Bearer ${env:token}\""
    );
    let mixed = Word {
        segs: vec![Seg::Text("$x `\"".into()), Seg::Env("t".into())],
    };
    assert_eq!(quoted(Shell::PowerShell, &mixed), "\"`$x ```\"${env:t}\"");
}

#[test]
fn cmd_escapes_every_character_it_would_read() {
    let q = |text: &str| quoted(Shell::Cmd, &Word::lit(text));
    assert_eq!(q("curl"), "curl");
    assert_eq!(q("https://x/a?b=1&c=2"), "^\"https://x/a?b=1^&c=2^\"");
    assert_eq!(q("{\"a\":\"b\"}"), "^\"{\\^\"a\\^\":\\^\"b\\^\"}^\"");
    assert_eq!(q("50% (off)"), "^\"50^% ^(off^)^\"");
    assert_eq!(q("C:\\dir\\"), "^\"C:\\dir\\\\^\"");
    assert_eq!(
        quoted(Shell::Cmd, &bearer()),
        "^\"Authorization: Bearer %token%^\""
    );
}

#[test]
fn a_cmd_script_doubles_its_percent_signs() {
    let mut renderer = Renderer::new(Shell::Cmd, true);
    assert_eq!(renderer.quote(&Word::lit("50%")), "^\"50%%^\"");
}

#[test]
fn cmd_puts_a_space_for_a_line_break_and_says_so() {
    let mut renderer = Renderer::new(Shell::Cmd, false);
    assert_eq!(renderer.quote(&Word::lit("a\r\nb")), "^\"a b^\"");
    assert!(renderer.altered);
    let mut posix = Renderer::new(Shell::Posix, false);
    assert_eq!(posix.quote(&Word::lit("a\nb")), "'a\nb'");
    assert!(!posix.altered);
}

#[test]
fn lines_continue_in_each_shells_way() {
    let groups = vec![
        vec![Word::lit("curl"), Word::lit("https://x/")],
        vec![Word::lit("--header"), Word::lit("A: b")],
    ];
    assert_eq!(
        Renderer::new(Shell::Posix, false).lines(&groups),
        "curl https://x/ \\\n  --header 'A: b'"
    );
    assert_eq!(
        Renderer::new(Shell::PowerShell, false).lines(&groups),
        "curl https://x/ `\n  --header 'A: b'"
    );
    assert_eq!(
        Renderer::new(Shell::Cmd, false).lines(&groups),
        "curl https://x/ ^\n  --header ^\"A: b^\""
    );
}
