use super::*;

#[test]
fn this_process_is_alive_and_the_largest_pid_is_not() {
    assert!(process_alive(std::process::id()));
    assert!(!process_alive(u32::MAX));
}

#[test]
fn a_staging_name_yields_its_pid_or_nothing() {
    let id = "a".repeat(32);
    assert_eq!(staging_pid(&format!("{id}-4242-0"), &id), Some(4242));
    assert_eq!(staging_pid(&format!("{id}-4242"), &id), None);
    assert_eq!(staging_pid(&format!("{id}-x-0"), &id), None);
    assert_eq!(staging_pid(&format!("{}-1-0", "b".repeat(32)), &id), None);
    assert_eq!(staging_pid(&id, &id), None);
}
