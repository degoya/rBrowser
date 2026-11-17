use super::{EMBEDDED_KEYS, EmbeddedKey, RELEASE_KEY_ID, root_keys_at};
use crate::sign::TrustStore;

static TABLE: &[EmbeddedKey] = &[
    EmbeddedKey {
        key_id: "old",
        public_key: "A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=",
        not_after: Some(100),
    },
    EmbeddedKey {
        key_id: "unconfigured",
        public_key: "",
        not_after: None,
    },
    EmbeddedKey {
        key_id: "current",
        public_key: "A6EHv/POEL4dcN0Y50vAmWfk1jCbpQ1fHdyGZBJVMbg=",
        not_after: None,
    },
];

#[test]
fn a_rotated_key_expires_and_an_empty_one_is_skipped() {
    let ids = |now| {
        root_keys_at(TABLE, now)
            .iter()
            .map(|entry| entry.key_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(50), ["old", "current"]);
    assert_eq!(ids(101), ["current"]);
}

#[test]
fn the_release_key_is_the_one_root_and_every_configured_root_decodes() {
    assert_eq!(EMBEDDED_KEYS.len(), 1);
    assert_eq!(EMBEDDED_KEYS[0].key_id, RELEASE_KEY_ID);
    TrustStore::with_roots().expect("every configured root decodes");
}
