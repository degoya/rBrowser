//! Files larger than one call's bytes (RB-099-24): `project-range.read` and `out-stream`.
//!
//! Both hold every rule of the interface they extend - `project.read`'s capability, path, trust
//! and grant checks, `out.write`'s capability and its folder `.rbrowser/out/<plugin-id>/`, no link
//! followed on the way - and the per-call limit [`MAX_IO_BYTES`] stays: a range reads, an append
//! takes at most that much. A streamed file grows in a hidden file beside its target
//! (`.<16 hex>.rbrowser-tmp`, made with `create_new`) and only `commit` renames it over the
//! target, so the target holds the old file or the whole new one. The [`Staged`] file lives in
//! the store of one plugin call (`runtime::PluginStoreState`): a `begin` drops the one before, a
//! failed `append` or `commit` drops it, and so does the end of the call however it ends - its
//! drop removes the hidden file.

use std::path::PathBuf;

use rand::Rng as _;
use rb_plugin_api::{MAX_IO_BYTES, MAX_STREAM_BYTES};
use rb_plugin_types::{Capability, Failure, FailureKind, codes, paths};
use tokio::io::AsyncWriteExt as _;

use super::{PluginContext, files, trust};
use crate::failure::too_large;

/// An `out-stream` file begun and not yet committed. The open file drops before the temporary
/// name is removed (field order), so Windows can delete it.
pub(crate) struct Staged {
    file: tokio::fs::File,
    temporary: files::Temporary,
    target: PathBuf,
    shown: String,
    written: u64,
}

impl std::fmt::Debug for Staged {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Staged")
            .field("shown", &self.shown)
            .field("written", &self.written)
            .finish_non_exhaustive()
    }
}

/// `out-stream.append` or `commit` with nothing begun in this call.
pub(crate) fn not_begun() -> Failure {
    Failure::coded(FailureKind::BadData, codes::INVALID).with_param("reason", "no file begun")
}

fn stream_limit() -> Failure {
    too_large(usize::try_from(MAX_STREAM_BYTES).unwrap_or(usize::MAX))
}

/// Refuses `target` when a link or a folder stands there: a streamed file never replaces one.
async fn plain_target(target: &std::path::Path, shown: &str) -> Result<(), Failure> {
    match files::kind(target)
        .await
        .map_err(|error| files::io(&error))?
    {
        Some(found) if found.is_symlink() || found.is_dir() => Err(files::outside(shown)),
        _ => Ok(()),
    }
}

impl PluginContext {
    /// `project-range.read`: `project.read`'s rules, `length` held to one call's limit; the
    /// file's own size is not limited.
    pub(crate) async fn project_read_range(
        &self,
        path: &str,
        offset: u64,
        length: u32,
    ) -> Result<(Vec<u8>, u64), Failure> {
        self.require(Capability::ProjectRead)?;
        let length = usize::try_from(length).unwrap_or(usize::MAX);
        if length > MAX_IO_BYTES {
            return Err(too_large(MAX_IO_BYTES));
        }
        let root = self.root()?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        trust::check(&segments, self.services.workspace_trusted(), path)?;
        self.check_readable(path, &segments)?;
        files::read_range(&root, &segments, path, offset, length).await
    }

    /// `out-stream.begin`: `out.write`'s path below `.rbrowser/out/<plugin-id>/` (folders made on
    /// the way, no link), a new hidden file beside it.
    pub(crate) async fn out_stream_begin(&self, path: &str) -> Result<Staged, Failure> {
        self.require(Capability::OutWrite)?;
        let root = self.root()?;
        let segments = paths::relative_segments(path).map_err(|refusal| refusal.failure(path))?;
        let mut full = vec![paths::CONFIG_DIR, paths::OUT_DIR, self.plugin_id.as_str()];
        full.extend(segments);
        let (name, folders) = full.split_last().ok_or_else(|| files::not_found(path))?;
        let folder = files::walk(&root, folders, true, path).await?;
        let target = folder.join(name);
        plain_target(&target, path).await?;
        let mut random = [0u8; 8];
        rand::rng().fill_bytes(&mut random);
        let temporary = files::Temporary(Some(folder.join(files::temporary_name(&random))));
        // `create_new`: never through anything planted at the name; only what it made is removed.
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(temporary.path())
            .await
            .map_err(|error| files::io(&error))?;
        Ok(Staged {
            file,
            temporary,
            target,
            shown: path.to_owned(),
            written: 0,
        })
    }

    /// `out-stream.append`: at most [`MAX_IO_BYTES`], the file at most [`MAX_STREAM_BYTES`].
    /// Flushed before it answers, so no write is still under way when the file is dropped.
    pub(crate) async fn out_stream_append(
        &self,
        staged: &mut Staged,
        bytes: &[u8],
    ) -> Result<(), Failure> {
        self.require(Capability::OutWrite)?;
        if bytes.len() > MAX_IO_BYTES {
            return Err(too_large(MAX_IO_BYTES));
        }
        let total = staged.written + bytes.len() as u64;
        if total > MAX_STREAM_BYTES {
            return Err(stream_limit());
        }
        let written = match staged.file.write_all(bytes).await {
            Ok(()) => staged.file.flush().await,
            Err(error) => Err(error),
        };
        written.map_err(|error| files::io(&error))?;
        staged.written = total;
        Ok(())
    }

    /// `out-stream.commit`: flushed to the disk, then renamed over the target at once.
    pub(crate) async fn out_stream_commit(&self, staged: Staged) -> Result<(), Failure> {
        self.require(Capability::OutWrite)?;
        let Staged {
            file,
            temporary,
            target,
            shown,
            ..
        } = staged;
        let synced = file.sync_all().await;
        drop(file);
        synced.map_err(|error| files::io(&error))?;
        // A link or folder put at the name since `begin` is refused, never replaced.
        plain_target(&target, &shown).await?;
        tokio::fs::rename(temporary.path(), &target)
            .await
            .map_err(|error| files::io(&error))?;
        temporary.renamed();
        Ok(())
    }
}

#[cfg(test)]
#[path = "large_tests.rs"]
mod tests;
