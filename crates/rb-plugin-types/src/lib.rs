//! The vocabulary rBrowser's plugins share with the host (RB-010-03, `docs/plugins.md`).
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-types` (itself from rDownloader's
//! `rd-plugin-types`): a [`Failure`] carries no English message, only a stable code - an i18n key
//! under `plugins.error.*` for the host's own, `plugin.<id>.*` for a plugin's - and its
//! parameters. New for rBrowser: the [`Capability`] a manifest grants, the [`PluginKind`] (one WIT
//! world each), the records the worlds exchange ([`Finding`], [`ViewInfo`], ...), the path rule
//! of `project-read` and `out-write` ([`paths`]), shared by the host and the native test host so
//! both refuse the same paths, and the findings model of every check run ([`MergedFinding`],
//! RB-060-01), and the one HTML escape of every report the app or a plugin writes ([`html`],
//! RB-080-28).

#![warn(unreachable_pub)]

mod capability;
pub mod codes;
mod failure;
mod finding;
pub mod html;
mod kind;
pub mod paths;
mod records;
pub mod rules;
mod view_engine;

pub use capability::{Capability, UnknownCapability};
pub use failure::{Failure, FailureKind, MessageParams};
#[cfg(feature = "schema")]
pub use finding::findings_schema_text;
pub use finding::{
    CheckFailure, CheckSkip, CheckTarget, CheckedView, ElementRef, FINDINGS_SCHEMA_FILE,
    FINDINGS_VERSION, FailOn, FindingMessage, FindingsFile, FindingsSummary, MergedFinding,
    SKIP_ENGINE, fingerprint, normalize_path, normalize_url,
};
pub use kind::{PluginKind, UnknownKind};
pub use records::{
    CheckContext, ExportFormat, ExportOutput, Finding, HttpRequest, HttpResponse, Proposal,
    RuleInfo, Severity, ViewId, ViewInfo,
};
