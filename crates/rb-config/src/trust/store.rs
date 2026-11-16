//! The trust store on disk, [`super::TRUST_FILE`]: read without a lock (it is replaced by a
//! rename, so a reader never sees half a file), changed under an exclusive lock on
//! `trust.json.lock` beside it, so two instances deciding at once lose no decision.

use std::fs::{self, OpenOptions};
use std::path::Path;

use super::{TRUST_STORE_VERSION, TrustStore};
use crate::ConfigError;
use crate::jsonio::{io_error, read_json, write_json};

/// See [`super::load_trust_store`].
pub fn load(path: &Path) -> Result<TrustStore, ConfigError> {
    if !path.exists() {
        return Ok(TrustStore {
            version: TRUST_STORE_VERSION,
            records: Vec::new(),
        });
    }
    let value = read_json(path)?;
    let store: TrustStore = serde_json::from_value(value).map_err(|error| ConfigError::Parse {
        path: path.to_path_buf(),
        line: 0,
        column: 0,
        message: error.to_string(),
    })?;
    if store.version > TRUST_STORE_VERSION {
        return Err(ConfigError::UnsupportedVersion {
            found: store.version,
            supported: TRUST_STORE_VERSION,
        });
    }
    Ok(TrustStore {
        version: TRUST_STORE_VERSION,
        records: store.records,
    })
}

/// See [`super::update_trust_store`].
pub fn update(
    path: &Path,
    change: impl FnOnce(&mut TrustStore),
) -> Result<TrustStore, ConfigError> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|source| io_error(dir, source))?;
    }
    let lock_path = path.with_file_name(format!(
        "{}.lock",
        path.file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&lock_path)
        .map_err(|source| io_error(&lock_path, source))?;
    lock.lock().map_err(|source| io_error(&lock_path, source))?;
    let result = load(path).and_then(|mut store| {
        change(&mut store);
        let value = serde_json::to_value(&store).expect("the trust store always serializes");
        write_json(path, &value).map(|()| store)
    });
    // Dropping the handle releases the lock as well; an unlock error changes nothing written.
    let _ = lock.unlock();
    result
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
