//! The instance pipe on Windows (RB-099-17): the names a second call accepts, the first instance
//! claimed by us, the DACL read back, and the same user getting through. Runs natively on
//! Windows only; `PIPE_REJECT_REMOTE_CLIENTS` needs a client on another machine and is not
//! tested here.

use std::path::Path;

use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, GENERIC_ALL};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;

use super::*;

/// The give-up deadline of a test that talks over the pipe, not a measurement.
const GIVE_UP: Duration = Duration::from_secs(10);

/// A pipe name of ours no other test uses: the random part differs on every call.
fn fresh_endpoint() -> String {
    endpoint_for(Path::new("."), "rb-test")
}

#[test]
fn a_name_of_ours_is_valid_and_new_each_time() {
    let endpoint = fresh_endpoint();
    assert!(valid_endpoint(&endpoint, Path::new(".")), "{endpoint}");
    assert_ne!(endpoint, fresh_endpoint());
}

/// Our prefix, then at least one letter, digit or `-`, at most 128 bytes in all; nothing that
/// leaves the pipe namespace or names another machine.
#[test]
fn only_our_prefix_and_plain_characters_are_valid() {
    let dir = Path::new(".");
    assert!(valid_endpoint(&format!("{PREFIX}a-1"), dir));
    assert!(!valid_endpoint(PREFIX, dir), "the prefix alone");
    assert!(!valid_endpoint("\\\\.\\pipe\\other-abc", dir));
    assert!(!valid_endpoint("\\\\server\\pipe\\rbrowser-abc", dir));
    assert!(!valid_endpoint("rbrowser-abc", dir));
    for rest in ["a\\b", "a/b", "..", "a.b", "a b", "a:b", "a\0b", "\u{4e16}"] {
        let endpoint = format!("{PREFIX}{rest}");
        assert!(!valid_endpoint(&endpoint, dir), "{endpoint:?}");
    }
    let at_limit = format!("{PREFIX}{}", "a".repeat(128 - PREFIX.len()));
    assert_eq!(at_limit.len(), 128);
    assert!(valid_endpoint(&at_limit, dir));
    assert!(!valid_endpoint(&format!("{at_limit}a"), dir), "129 bytes");
}

/// `FILE_FLAG_FIRST_PIPE_INSTANCE`: while we hold the name nobody binds it again, a process
/// that came first included; once we let go it is free.
#[tokio::test]
async fn a_second_bind_on_the_same_name_fails() {
    let endpoint = fresh_endpoint();
    let first = bind(&endpoint).expect("the first instance");
    let Err(error) = bind(&endpoint) else {
        panic!("a second bind on {endpoint} succeeded");
    };
    assert_eq!(
        error.raw_os_error(),
        i32::try_from(ERROR_ACCESS_DENIED).ok(),
        "{error}"
    );
    drop(first);
    bind(&endpoint).expect("the name is free once the first instance is gone");
}

/// Protected, one allow entry for the token's user: the first instance and every further one
/// the listener makes. The system may keep `GA` as written or map it to the file rights.
#[tokio::test]
async fn every_instance_grants_the_user_alone() {
    let endpoint = fresh_endpoint();
    let listener = bind(&endpoint).expect("bound");
    let further = create(&endpoint, false).expect("a further instance");
    let user = rb_process::current_user_sid().expect("the token has a user");
    for instance in [&listener.next, &further] {
        let read = rb_process::dacl::of_kernel_object(instance).expect("the DACL reads back");
        assert!(read.protected, "{read:?}");
        assert_eq!(read.aces.len(), 1, "{read:?}");
        let ace = &read.aces[0];
        assert_eq!(ace.kind, rb_process::dacl::ACCESS_ALLOWED, "{read:?}");
        assert_eq!(ace.sid, user, "{read:?}");
        assert!(
            ace.mask == GENERIC_ALL || ace.mask & FILE_ALL_ACCESS == FILE_ALL_ACCESS,
            "{:#x}",
            ace.mask
        );
    }
}

/// The user the DACL grants gets through: a byte goes over and comes back.
#[tokio::test]
async fn the_same_user_connects_and_talks() {
    let endpoint = fresh_endpoint();
    let mut listener = bind(&endpoint).expect("bound");
    let server = tokio::spawn(async move {
        let mut conn = listener.accept().await.expect("a client");
        let mut byte = [0_u8; 1];
        conn.read_exact(&mut byte).await.expect("a byte");
        conn.write_all(&byte).await.expect("echoed");
        conn.flush().await.expect("flushed");
        // Kept until the client has read the echo.
        conn
    });
    tokio::time::timeout(GIVE_UP, async {
        let mut client = connect(&endpoint).await.expect("connected");
        client.write_all(&[42]).await.expect("sent");
        let mut back = [0_u8; 1];
        client.read_exact(&mut back).await.expect("read back");
        assert_eq!(back, [42]);
        server.await.expect("the server task ends");
    })
    .await
    .expect("the round trip ended");
}
