use super::matches;

#[test]
fn double_star_crosses_folders_and_star_does_not() {
    assert!(matches("**/*.todo", "TODO.todo"));
    assert!(matches("**/*.todo", "docs/plan/next.todo"));
    assert!(matches("*.todo", "TODO.todo"));
    assert!(!matches("*.todo", "docs/TODO.todo"));
    assert!(matches("docs/**/*.todo", "docs/a/b.todo"));
    assert!(matches("docs/**/*.todo", "docs/b.todo"));
    assert!(!matches("docs/**/*.todo", "src/b.todo"));
    assert!(matches("docs/?.todo", "docs/a.todo"));
    assert!(!matches("docs/?.todo", "docs/ab.todo"));
    assert!(matches("a*b*c.todo", "axxbyyc.todo"));
    assert!(!matches("**/*.todo", "notes.todo.bak"));
    assert!(matches("**", "any/thing"));
}
