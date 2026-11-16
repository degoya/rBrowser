use super::*;

#[test]
fn finds_the_context_of_a_frame_and_the_frame_of_a_context() {
    let mut table = ContextTable::default();
    table.insert("S1", "F-main", 7);
    table.insert("S1", "F-child", 9);
    assert_eq!(table.context_of("S1", "F-main"), Some(7));
    assert_eq!(table.frame_of("S1", 9), Some("F-child"));
    assert_eq!(table.context_of("S2", "F-main"), None);
}

#[test]
fn a_new_document_replaces_the_frame_s_context() {
    let mut table = ContextTable::default();
    table.insert("S1", "F", 7);
    table.insert("S1", "F", 8);
    assert_eq!(table.context_of("S1", "F"), Some(8));
    assert_eq!(table.contexts("S1"), vec![8]);
}

#[test]
fn destroyed_and_cleared_contexts_are_gone() {
    let mut table = ContextTable::default();
    table.insert("S1", "F1", 7);
    table.insert("S1", "F2", 8);
    table.insert("S2", "F3", 9);
    table.remove("S1", 7);
    assert_eq!(table.contexts("S1"), vec![8]);
    table.clear("S1");
    assert!(table.contexts("S1").is_empty());
    assert_eq!(table.contexts("S2"), vec![9]);
}
