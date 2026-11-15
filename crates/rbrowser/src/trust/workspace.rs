//! The plugin host's [`WorkspaceTrust`] over `rb_config::trust`: whether a project's
//! `.rbrowser/plugins/` may load, decided afresh from the files and the store on every call,
//! so a changed plugin after the decision is not trusted.

use std::path::{Path, PathBuf};

use rb_config::trust::{ProtectedKind, trust_state};
use rb_plugin_host::WorkspaceTrust;

use super::state::load_store;

/// Trust as the person decided it in the shell; the coordinator hands it to the plugin loader.
#[derive(Clone, Debug)]
pub struct ProjectTrust {
    /// `trust.json`; `None` trusts nothing.
    store: Option<PathBuf>,
}

impl ProjectTrust {
    /// Over the store at `store` (see `super::state::store_path`).
    pub fn new(store: Option<PathBuf>) -> Self {
        Self { store }
    }
}

impl WorkspaceTrust for ProjectTrust {
    fn is_trusted(&self, project_root: &Path) -> bool {
        if self.store.is_none() {
            return false;
        }
        let store = load_store(self.store.as_deref());
        trust_state(project_root, &store).is_ok_and(|state| state.allows(ProtectedKind::Plugins))
    }
}
