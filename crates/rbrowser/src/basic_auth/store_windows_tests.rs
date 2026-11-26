//! The Credential Manager store on Windows (RB-099-17): write, read, replace and delete against
//! the real store of the user running the tests. Every target lies in a namespace of its own
//! (`rbrowser-test/basic-auth/<process>-<random>/...`), never under `store::SERVICE`,
//! and a guard deletes it again when the test ends, a failed one included. Runs natively on
//! Windows only.

use super::*;

/// A target name no real credential and no other test uses, deleted on drop.
struct TestTarget(String);

impl TestTarget {
    fn new(name: &str) -> Self {
        let random = crate::secret::random_hex(8).expect("a random source");
        Self(format!(
            "rbrowser-test/basic-auth/{}-{random}/{name}",
            std::process::id()
        ))
    }
}

impl Drop for TestTarget {
    fn drop(&mut self) {
        // Cleanup on every way out, a panic included; nothing to report when it is gone.
        let _ = WindowsStore.remove(&self.0);
    }
}

fn credential(user: &str, password: &str) -> Credential {
    Credential::new(user, password).expect("a credential Basic auth carries")
}

#[test]
fn a_credential_is_written_read_and_deleted() {
    let target = TestTarget::new("round-trip");
    let written = credential("tester", "s3cret-pass");
    WindowsStore
        .write(&target.0, &written)
        .expect("the store keeps it");
    assert_eq!(WindowsStore.read(&target.0), Ok(Some(written)));
    assert_eq!(WindowsStore.remove(&target.0), Ok(true));
    assert_eq!(WindowsStore.read(&target.0), Ok(None));
    assert_eq!(WindowsStore.remove(&target.0), Ok(false), "already gone");
}

#[test]
fn a_second_write_replaces_the_first() {
    let target = TestTarget::new("replace");
    WindowsStore
        .write(&target.0, &credential("first", "first-pass"))
        .expect("written");
    let second = credential("second", "second-pass");
    WindowsStore.write(&target.0, &second).expect("replaced");
    assert_eq!(WindowsStore.read(&target.0), Ok(Some(second)));
}

/// The user name travels as UTF-16, the password as the UTF-8 blob: both come back unchanged.
#[test]
fn text_beyond_ascii_comes_back_unchanged() {
    let target = TestTarget::new("unicode");
    let written = credential("user-\u{4e16}\u{754c}", "p\u{00e4}ss-\u{1f511}-word");
    WindowsStore.write(&target.0, &written).expect("written");
    assert_eq!(WindowsStore.read(&target.0), Ok(Some(written)));
}

#[test]
fn a_target_never_written_is_nothing() {
    let target = TestTarget::new("missing");
    assert_eq!(WindowsStore.read(&target.0), Ok(None));
    assert_eq!(WindowsStore.remove(&target.0), Ok(false));
}

/// The targets of one test do not reach each other's credential.
#[test]
fn targets_are_kept_apart() {
    let first = TestTarget::new("first");
    let second = TestTarget::new("second");
    WindowsStore
        .write(&first.0, &credential("tester", "s3cret-pass"))
        .expect("written");
    assert_eq!(WindowsStore.read(&second.0), Ok(None));
    assert_eq!(WindowsStore.remove(&second.0), Ok(false));
    assert!(WindowsStore.read(&first.0).expect("read").is_some());
}
