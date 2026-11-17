//! rBrowser's plugin host (RB-010-03, `docs/plugins.md`): the Wasmtime engine, the limits of a
//! store and a call, the compile cache, the manifest, the `.rbplugin` package and its signature,
//! the capabilities behind every host function, and the contributions a plugin registers.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host` (itself from rDownloader's
//! `rd-plugin-host`); every module names its source. Plugins are signed WebAssembly components:
//! no WASI, no file system, no network of their own - only `host`, their settings and the one
//! interface per capability their manifest grants. A trap fails the one call; memory is capped
//! per store, fuel and an epoch deadline per call; a plugin that keeps failing is suspended.
//!
//! What the application wires (each a trait here, the hook points of the soft dependencies):
//! [`HostServices`] (views, CDP, HTTP, sidecar, findings, progress), [`WorkspaceTrust`] (RB-010-09: project
//! plugins load only under workspace trust), [`LocaleSink`] (RB-010-14: `rb_i18n`'s plugin
//! locales) and [`SettingsSink`] (RB-010-02: the plugin's settings fragment). Commands go into
//! `rb_commands::CommandRegistry` as [`plugin_command_defs`] and [`core_command_defs`] build them.

#![warn(unreachable_pub)]

mod bindings;
mod bridge;
mod call_gate;
mod cdp_policy;
mod checks;
mod commands;
mod compile_cache;
mod conformance;
mod contents;
mod context;
mod engine;
mod failure;
mod health;
mod index;
mod limits;
mod link;
mod loader;
mod log_budget;
mod manifest;
mod package;
mod packager;
mod plugin;
mod rate;
mod recover;
mod refusal;
mod registry;
mod repository;
mod runtime;
mod services;
mod sign;
mod sinks;

/// Test components, signed packages and a scripted service backend for the tests of this crate
/// and the crates above; never in a shipped build.
#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
#[cfg(any(test, feature = "test-support"))]
pub mod testbed;

pub use bridge::{
    BUSY, LogLevel, MAX_LOG_CHARS, MAX_LOG_VALUES, MAX_MESSAGE_BYTES, MAX_PENDING, MAX_POSTS,
    MAX_STACK_CHARS, NETWORK_RULES_TOOL, NO_ANSWER, PAGE_MESSAGE_TOOL, PAGE_TIMEOUT_MILLISECONDS,
    POST_KEY, PageBridge, PageInput, PageLog, PageRefusal, PageScriptSource, Post, PostTarget,
    STOPPED, page_request_args, parse_page_payload, post_message, receiver_call, reply_message,
    split_posts, stop_message, world_name,
};
pub use call_gate::CallSlot;
pub use cdp_policy::PLUGIN_CDP_METHODS;
pub use checks::{
    CHECKS_DIR, CHECKS_RUN_DESCRIPTION, CHECKS_RUN_TOOL, Cancel, CheckRunner, CheckSource,
    ChecksPolicy, ChecksRunAnswer, ChecksRunArgs, ChecksSettings, DEFAULT_KEEP_RUNS,
    DEFAULT_PARALLEL, IgnorePattern, JobProgress, JobState, MAX_ANSWER_FINDINGS, MAX_KEEP_RUNS,
    MAX_PARALLEL, PluginTiming, ProgressSink, Report, RuleLevel, RunControl, RunOutcome,
    RunRequest, RunView, SkipReason, estimate_ms, findings_path, merge, reported_run, run_id,
    timings_of, valid_run_id, write_run,
};
pub use commands::{
    CORE_COMMANDS, PANELS_COMMAND, PANELS_REASON, PLUGINS_CATALOG_TOOL, PLUGINS_LIST_TOOL,
    PROTECTED_REASON, core_command_defs, plugin_command_defs,
};
pub use compile_cache::sha256_hex;
pub use conformance::{ConformanceCheck, ConformanceReport, run_conformance};
pub use contents::{check_locale, check_settings_schema};
pub use context::PluginContext;
pub use engine::configure_compile_cache;
pub use failure::readable;
pub use health::{
    DEFAULT_CRASH_LIMIT, FAILURE_WINDOW, MAX_CRASH_LIMIT, MIN_CRASH_LIMIT, PluginHealth,
    PluginHealths,
};
// The signed plugin index of the catalogue (RB-091-01).
pub use index::{
    ALGORITHM_ED25519, CatalogEntry, Compatibility, DEFAULT_VALIDITY_DAYS, DocumentSignature,
    Freshness, INDEX_FILE_NAME, IndexEntry, IndexError, IndexSigners, MAX_CLOCK_SKEW_MINUTES,
    MAX_INDEX_BYTES, MAX_INDEX_PLUGINS, MAX_RELEASE_NOTES_CHARS, MAX_VALIDITY_DAYS, Offer,
    PLUGIN_INDEX_DOMAIN, PLUGIN_INDEX_SCHEMA_VERSION, PluginIndex, Publisher, SignatureError,
    SignedDocument, StaleError, check_freshness, compatibility, describe_package, merge_offers,
    package_file_name, sign as sign_index, sign_document, update_for, verify as verify_index,
};
pub use limits::{
    DEFAULT_FUEL, DEFAULT_MAX_CONCURRENT, DEFAULT_MAX_RESPONSE_BYTES, DEFAULT_MEMORY_BYTES,
    DEFAULT_TIMEOUT_MILLISECONDS, MAX_CONCURRENT, MAX_FUEL, MAX_MEMORY_BYTES, MAX_RESPONSE_BYTES,
    MAX_TIMEOUT_MILLISECONDS, PluginLimits,
};
pub use loader::{
    DisabledEntry, LoadContext, LoadReport, LoadedEntry, PluginSource, Refused, Sources,
    declared_capabilities, declared_grants, discover, load_all, project_plugins_dir,
};
pub use log_budget::MAX_LOG_LINES_PER_CALL;
pub use manifest::{
    CliEntry, CommandEntry, DEFAULT_HTTP_RATE_PER_SECOND, Grants, MANIFEST_VERSION,
    MAX_HTTP_RATE_PER_SECOND, MAX_MANIFEST_BYTES, Metadata, PLUGIN_SIDECAR_NAMESPACES, PageScript,
    Panel, PanelPlacement, PluginManifest, RESERVED_IDS, RESERVED_MCP_PREFIXES,
    SETTINGS_SCHEMA_MEMBER, SUPPORTED_API_VERSIONS, ToolEntry, parse_manifest, valid_mcp_prefix,
    valid_plugin_id,
};
pub use package::{
    COMPONENT_MEMBER, MANIFEST_MEMBER, MAX_ASSET_BYTES, MAX_COMPONENT_BYTES, MAX_FILES,
    MAX_FILES_BYTES, MAX_PACKAGE_BYTES, MAX_SIGNATURE_BYTES, MAX_TEXT_MEMBER_BYTES,
    PACKAGE_EXTENSION, SIGNATURE_MEMBER, VerifiedPackage, package_digest,
};
pub use packager::{
    GeneratedKey, generate_signing_key, load_signing_key_pem, package_plugin, public_key_base64,
    verify_package,
};
pub use plugin::{LoadedPlugin, PluginOptions};
pub use refusal::{Refusal, refusal_code, refusal_failure};
pub use registry::{PluginTool, Registry};
// The plugin catalogue's repositories, refresh and install (RB-091-01).
#[cfg(any(test, feature = "test-support"))]
pub use repository::MemoryFetcher;
pub use repository::{
    CATALOG_DIR, CONNECT_TIMEOUT_SECONDS, Catalog, CatalogStore, Consent, FLOORS_FILE, Fetcher,
    HttpFetcher, INDEX_TIMEOUT_SECONDS, INSTALLS_FILE, InstallRecord, Installed, KeyReview,
    LoadedIndex, MAX_REDIRECTS, MAX_USER_REPOSITORIES, OFFICIAL_INDEX_URL, OFFICIAL_REPOSITORY,
    PACKAGE_TIMEOUT_SECONDS, PinnedKey, Prepared, Repository, RepositoryError,
    download as download_package, install as install_package, parse_https,
    prepare as prepare_package, remove as remove_package, user_agent,
};
pub use services::{Caller, HostServices, NoServices, not_available};
pub use sign::{
    DigestBuilder, EMBEDDED_KEYS, EmbeddedKey, RELEASE_KEY_ID, TrustStore, decode_public_key,
    hex_sha256, key_fingerprint, root_keys, root_keys_at, sign_detached, verify_detached,
    verify_detached_bytes,
};
pub use sinks::{DenyAllTrust, LocaleSink, NoLocales, NoSettings, SettingsSink, WorkspaceTrust};
pub use verifier::{APP_VERSION, PluginVerifier, VerifyError};

mod verifier;
