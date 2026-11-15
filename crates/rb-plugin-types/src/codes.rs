//! The host's failure codes: each an i18n key under `plugins.error.*` (the English text is in
//! `locales/parts/RB-010-03.json`, merged into `locales/en.json`). A plugin's own codes are keys of
//! its locales, `plugin.<id>.*`.
//!
//! Copied and adapted from rCommander's `crates/rc-plugin-host/src/failure.rs` (`codes`): the same
//! process-model codes (crashed, timed out, out of fuel or memory, denied, suspended), written as
//! keys, and the codes of rBrowser's capabilities, packages and contributions.

/// A trap: `unreachable`, a panic, an out-of-bounds access.
pub const CRASHED: &str = "plugins.error.crashed";
/// The epoch deadline (or the caller's wall clock) ended the call.
pub const TIMED_OUT: &str = "plugins.error.timed_out";
/// The call spent its instruction budget.
pub const OUT_OF_FUEL: &str = "plugins.error.out_of_fuel";
/// The store asked for more memory than the manifest allows.
pub const OUT_OF_MEMORY: &str = "plugins.error.out_of_memory";
/// The plugin failed repeatedly and is suspended until a person resumes it.
pub const SUSPENDED: &str = "plugins.error.suspended";
/// The caller stopped the call.
pub const CANCELLED: &str = "plugins.error.cancelled";
/// The plugin answered something the host refuses; `what` names the rule.
pub const BAD_OUTPUT: &str = "plugins.error.bad_output";
/// The host could not set a call up (logged, never shown in detail).
pub const HOST_FAILED: &str = "plugins.error.host_failed";

/// A host function the manifest does not grant; `capability` names the capability.
pub const CAPABILITY_DENIED: &str = "plugins.error.capability_denied";
/// The component imports an interface no granted capability covers; `import` names it.
pub const DENIED: &str = "plugins.error.denied";
/// A CDP method of a domain the manifest's `cdp.domains` does not list; `domain` names it.
pub const CDP_DOMAIN_DENIED: &str = "plugins.error.cdp_domain_denied";
/// A path that leaves the plugin's area (`..`, an absolute path, a link out); `path` names it.
pub const PATH_OUTSIDE: &str = "plugins.error.path_outside";
/// A path `project-read` does not cover: neither `.rbrowser/` nor a declared glob.
pub const PATH_NOT_GRANTED: &str = "plugins.error.path_not_granted";
/// The file or directory does not exist.
pub const NOT_FOUND: &str = "plugins.error.not_found";
/// A read, write or answer larger than the host allows; `limit` names the bound.
pub const TOO_LARGE: &str = "plugins.error.too_large";
/// A URL the `http` capability does not fetch (not `http:` or `https:`); `url` names it.
pub const HTTP_URL_DENIED: &str = "plugins.error.http_url_denied";
/// A Lighthouse run (`lighthouse.start`) on an address outside the sites of the views, while the
/// person has not allowed any address (`plugins.lighthouseAnyUrl`, RB-090-21); `url` names it.
pub const SIDECAR_URL_DENIED: &str = "plugins.error.sidecar_url_denied";
/// The request failed on the way (no answer, broken connection).
pub const HTTP_FAILED: &str = "plugins.error.http_failed";
/// No backend is wired for the service (views, CDP, sidecar) in this process.
pub const NOT_AVAILABLE: &str = "plugins.error.not_available";
/// A command or tool the plugin does not declare; `name` names it.
pub const UNKNOWN_CONTRIBUTION: &str = "plugins.error.unknown_contribution";

/// The package or its manifest is broken; `reason` says what, in English (a log line).
pub const INVALID: &str = "plugins.error.invalid";
/// An unsigned package outside development mode.
pub const UNSIGNED: &str = "plugins.error.unsigned";
/// The signature does not verify: the package was changed after it was signed.
pub const SIGNATURE_INVALID: &str = "plugins.error.signature_invalid";
/// Signed by a key this installation does not trust yet; `fingerprint` names it.
pub const KEY_UNTRUSTED: &str = "plugins.error.key_untrusted";
/// The manifest's key differs from the key trusted under its `key_id`.
pub const KEY_MISMATCH: &str = "plugins.error.key_mismatch";
/// The plugin is built against a contract this rBrowser does not run; `api_version` names it.
pub const API_VERSION: &str = "plugins.error.api_version";
/// The plugin needs a newer rBrowser; `min_app_version` names it.
pub const NEEDS_NEWER_APP: &str = "plugins.error.needs_newer_app";
/// The component does not export what its declared kinds require; `kind` names the world.
pub const WORLD_MISSING: &str = "plugins.error.world_missing";

/// Another plugin has this id; `id` names it.
pub const DUPLICATE_ID: &str = "plugins.error.duplicate_id";
/// Another plugin holds this MCP prefix; `prefix` and `other` name them.
pub const MCP_PREFIX_TAKEN: &str = "plugins.error.mcp_prefix_taken";
/// The MCP prefix is a core tool area (`settings`, `views`, ...); `prefix` names it.
pub const MCP_PREFIX_RESERVED: &str = "plugins.error.mcp_prefix_reserved";
/// The settings schema names a category rBrowser does not have; `category` and `key` name them.
pub const SETTINGS_CATEGORY: &str = "plugins.error.settings_category";
/// The settings schema names a section its category does not have; `section` and `key`.
pub const SETTINGS_SECTION: &str = "plugins.error.settings_section";
/// A settings key without `x-category`, `x-section` or `x-order`; `key` and `annotation`.
pub const SETTINGS_ANNOTATION: &str = "plugins.error.settings_annotation";
/// A project plugin in a workspace the person has not trusted.
pub const UNTRUSTED_WORKSPACE: &str = "plugins.error.untrusted_workspace";

/// A notice, not a refusal: a plugin's default shortcut is reserved or taken; the command stays
/// unbound and reachable through the leader and the palette. `command` and `binding` name them.
pub const SHORTCUT_UNBOUND: &str = "plugins.notice.shortcut_unbound";

/// Every code above, for the locale lint and the tests.
pub const ALL: &[&str] = &[
    CRASHED,
    TIMED_OUT,
    OUT_OF_FUEL,
    OUT_OF_MEMORY,
    SUSPENDED,
    CANCELLED,
    BAD_OUTPUT,
    HOST_FAILED,
    CAPABILITY_DENIED,
    DENIED,
    CDP_DOMAIN_DENIED,
    PATH_OUTSIDE,
    PATH_NOT_GRANTED,
    NOT_FOUND,
    TOO_LARGE,
    HTTP_URL_DENIED,
    SIDECAR_URL_DENIED,
    HTTP_FAILED,
    NOT_AVAILABLE,
    UNKNOWN_CONTRIBUTION,
    INVALID,
    UNSIGNED,
    SIGNATURE_INVALID,
    KEY_UNTRUSTED,
    KEY_MISMATCH,
    API_VERSION,
    NEEDS_NEWER_APP,
    WORLD_MISSING,
    DUPLICATE_ID,
    MCP_PREFIX_TAKEN,
    MCP_PREFIX_RESERVED,
    SETTINGS_CATEGORY,
    SETTINGS_SECTION,
    SETTINGS_ANNOTATION,
    UNTRUSTED_WORKSPACE,
    SHORTCUT_UNBOUND,
];
