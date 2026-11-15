use super::*;

#[test]
fn a_generated_key_reads_back_and_is_never_overwritten() {
    let folder = tempfile::tempdir().expect("tempdir");
    let public = generate(folder.path()).expect("keygen");
    let (private_file, public_file) = key_files(folder.path());
    let key = crate::build::read_key_file(&private_file).expect("read the key");
    assert_eq!(STANDARD.encode(key.verifying_key().as_bytes()), public);
    assert_eq!(
        std::fs::read_to_string(&public_file)
            .expect("public")
            .trim(),
        public
    );
    assert!(generate(folder.path()).is_err(), "never overwritten");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&private_file)
            .expect("meta")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn two_runs_give_two_different_keys() {
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");
    assert_ne!(
        generate(first.path()).expect("first"),
        generate(second.path()).expect("second")
    );
}
