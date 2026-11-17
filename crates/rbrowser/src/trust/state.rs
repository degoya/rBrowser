//! The trust of the open project in the app: the store beside the user settings, the check
//! that writes [`crate::host::shell_state::ShellState::trusted`], the open prompts and the
//! person's decisions.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use rb_config::trust::{
    ProtectedKind, TrustState, TrustStore, load_trust_store, trust_state, trust_store_for,
    update_trust_store,
};
use rb_engine::ViewHost;
use rb_ipc::{
    CoreToShell, IpcErrorCode, Notification, NotificationLevel, TrustDecision, TrustInfo,
    TrustPrompt, TrustStatus, TrustedProject,
};

use crate::cdp::lock;
use crate::control::Core;
use crate::ipc::Failure;

/// The notice after a decision that changes the network keys: they are switches of the
/// browser process and change at the next start.
pub const RESTART_NOTICE_KEY: &str = "trust.notice.network_restart";

/// How many prompts stay answerable; older ones are forgotten.
const MAX_OPEN_PROMPTS: usize = 16;

/// A question shown to the person: the root and the hash of exactly what was listed.
#[derive(Clone, Debug, PartialEq, Eq)]
struct OpenPrompt {
    root: PathBuf,
    hash: String,
}

struct Prompts {
    next: u32,
    open: BTreeMap<u32, OpenPrompt>,
}

/// One window per process: the prompts live beside the core.
static PROMPTS: Mutex<Prompts> = Mutex::new(Prompts {
    next: 1,
    open: BTreeMap::new(),
});

/// `trust.json` beside the user settings this core writes (portable mode included), by the
/// same rule as `rb_config::trust::trust_store_path`, which the start-up and the plugin loader
/// read; `None` without a user settings file - then nothing is ever trusted.
pub fn store_path(core: &Core) -> Option<PathBuf> {
    core.user_settings_path().map(trust_store_for)
}

/// The store at `path`; a missing path, a broken or a newer file trust nothing.
pub fn load_store(path: Option<&Path>) -> TrustStore {
    let Some(path) = path else {
        return TrustStore::default();
    };
    load_trust_store(path).unwrap_or_else(|error| {
        tracing::warn!(%error, "the workspace trust store cannot be read; nothing is trusted");
        TrustStore::default()
    })
}

/// The open project's root and trust as the files are now; `None` without a project. A check
/// that fails counts as "not trusted, nothing listed".
pub fn current(core: &Core) -> Option<(PathBuf, TrustState)> {
    let host = core.host();
    let root = host.project_root()?;
    let store = load_store(store_path(core).as_deref());
    let state = trust_state(&root, &store).unwrap_or_else(|error| {
        tracing::warn!(%error, "the workspace trust cannot be checked; the project stays restricted");
        TrustState::default()
    });
    Some((root, state))
}

/// Checks the open project again and writes the shell state; when the trust changed, every
/// view gets its emulation again (project headers come or go), the project plugins run or stop
/// and, if network keys are among the parts, the person learns they change at the next start.
pub fn refresh(core: &Core) -> Option<(PathBuf, TrustState)> {
    // One check at a time (RB-090-36 review): the start's check runs off the UI thread and a
    // config change or a decision may come meanwhile; two at once would both see the trust
    // change and reload the plugins and prompt twice.
    static ONE_CHECK: Mutex<()> = Mutex::new(());
    let _one_at_a_time = lock(&ONE_CHECK);
    let (root, state) = current(core)?;
    let host = core.host();
    let before = host.shell_state().trusted;
    host.update_shell_state(|shell| shell.trusted = state.trusted);
    if before != state.trusted {
        tracing::info!(
            trusted = state.trusted,
            parts = state.blocked.len(),
            "workspace trust changed"
        );
        for view in host.views() {
            host.reapply_emulation(view.id);
        }
        // The project plugins run or stop with it, at once (review M2 of Charge 6).
        crate::plugins::on_trust_changed(core);
        // The Lighthouse bar shows the runs of a trusted project only (RB-090-73).
        crate::view_footer::lighthouse_runs::reload(core);
        // A trusted project's extension offer appears, an untrusted one's goes (RB-080-06).
        crate::extensions::on_config_changed(core);
        if has_network_keys(&state) {
            core.notify(CoreToShell::Notification(Notification {
                level: NotificationLevel::Info,
                message_key: RESTART_NOTICE_KEY.to_owned(),
                args: BTreeMap::new(),
                settings_key: None,
            }));
        }
    }
    Some((root, state))
}

fn has_network_keys(state: &TrustState) -> bool {
    state.blocked.iter().any(|item| {
        matches!(
            item.kind,
            ProtectedKind::HostResolverRules
                | ProtectedKind::IgnoreCertificateErrors
                | ProtectedKind::Proxy
        )
    })
}

/// Asks the person when the project has protected parts it is not trusted with: a
/// `trustPrompt` for the banner (kept until the shell is ready). An open prompt for the same
/// root and parts is asked again under its id.
pub fn prompt_if_needed(core: &Core, root: &Path, state: &TrustState) {
    if state.trusted || state.blocked.is_empty() {
        return;
    }
    send_prompt(core, root, state);
}

/// Asks on the person's own request (`trust.grant`, the dialog): as [`prompt_if_needed`], and
/// also for a project with no protected part - trusting it opens its own origins to the
/// plugins' `http` (review finding L2 of Charge 6). A project with a link out of it cannot be
/// trusted and is not asked.
pub fn prompt_on_request(core: &Core, root: &Path, state: &TrustState) {
    if state.trusted || !state.unpinned.is_empty() {
        return;
    }
    send_prompt(core, root, state);
}

fn send_prompt(core: &Core, root: &Path, state: &TrustState) {
    let prompt_id = open_prompt(root, &state.hash);
    core.notify(CoreToShell::TrustPrompt(TrustPrompt {
        prompt_id,
        project_root: root.to_string_lossy().into_owned(),
        blocked: state.blocked.clone(),
    }));
}

fn open_prompt(root: &Path, hash: &str) -> u32 {
    let wanted = OpenPrompt {
        root: root.to_path_buf(),
        hash: hash.to_owned(),
    };
    let mut prompts = lock(&PROMPTS);
    if let Some((id, _)) = prompts.open.iter().find(|(_, open)| **open == wanted) {
        return *id;
    }
    let id = prompts.next;
    prompts.next = prompts.next.wrapping_add(1).max(1);
    prompts.open.insert(id, wanted);
    while prompts.open.len() > MAX_OPEN_PROMPTS {
        prompts.open.pop_first();
    }
    id
}

/// The dialog's answer (`trustDecision`), the only way trust is granted. Trusting stores the
/// root with the hash of what the prompt listed; if the parts changed since, the check asks
/// again at once.
pub fn decide(core: &Core, prompt_id: u32, decision: TrustDecision) -> Result<(), Failure> {
    let prompt = lock(&PROMPTS).open.remove(&prompt_id).ok_or_else(|| {
        (
            IpcErrorCode::NotFound,
            format!("no open trust prompt {prompt_id}"),
        )
    })?;
    if decision == TrustDecision::Restricted {
        tracing::info!("the project stays restricted");
        return Ok(());
    }
    let open_root = core.host().project_root();
    if open_root.as_deref() != Some(prompt.root.as_path()) {
        return Err((
            IpcErrorCode::BadRequest,
            "the trust prompt belongs to a project that is no longer open".to_owned(),
        ));
    }
    if let Some((_, state)) = current(core).filter(|(_, state)| !state.unpinned.is_empty()) {
        return Err((
            IpcErrorCode::BadRequest,
            format!(
                "links in protected folders lead out of the project or nowhere, so it cannot be \
                 trusted: {}",
                state.unpinned.join(", ")
            ),
        ));
    }
    let path = store_path(core).ok_or_else(|| {
        (
            IpcErrorCode::Internal,
            "there is no user settings folder to keep the decision in".to_owned(),
        )
    })?;
    let decided_at = rfc3339(SystemTime::now());
    update_trust_store(&path, |store| {
        store.grant(&prompt.root, &prompt.hash, &decided_at);
    })
    .map_err(|error| (IpcErrorCode::Internal, error.to_string()))?;
    if let Some((root, state)) = refresh(core) {
        prompt_if_needed(core, &root, &state);
    }
    Ok(())
}

/// Withdraws the trust of the project at `root`; when it is the open one, its protected parts
/// stop at once and the banner asks again.
pub fn revoke(core: &Core, root: &Path) -> Result<(), Failure> {
    let path = store_path(core).ok_or_else(|| {
        (
            IpcErrorCode::Internal,
            "there is no user settings folder with a trust store".to_owned(),
        )
    })?;
    update_trust_store(&path, |store| {
        store.revoke(root);
    })
    .map_err(|error| (IpcErrorCode::Internal, error.to_string()))?;
    if let Some((open, state)) = refresh(core) {
        prompt_if_needed(core, &open, &state);
    }
    Ok(())
}

/// The `trust` answer: the open project's trust and every trusted project.
pub fn info(core: &Core) -> CoreToShell {
    let store = load_store(store_path(core).as_deref());
    let current = current(core).map(|(root, state)| {
        let declared = super::grants::declared(&root, &state.blocked);
        TrustStatus {
            project_root: root.to_string_lossy().into_owned(),
            trusted: state.trusted,
            plugin_capabilities: super::grants::capabilities(&declared),
            plugin_grants: super::grants::details_of(&declared),
            blocked: state.blocked,
        }
    });
    let projects = store
        .records
        .into_iter()
        .map(|record| TrustedProject {
            root: record.root,
            decided_at: record.decided_at,
        })
        .collect();
    CoreToShell::Trust(TrustInfo { current, projects })
}

/// `time` as RFC 3339 UTC, to the second.
pub fn rfc3339(time: SystemTime) -> String {
    let seconds = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    let days = i64::try_from(seconds / 86_400).unwrap_or(0);
    let (year, month, day) = rb_diag::cef::civil_from_days(days);
    let rest = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest / 60 % 60,
        rest % 60
    )
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
