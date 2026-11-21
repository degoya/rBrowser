//! [`Component`]: what the plugin runtime ([`super::runtime`]) asks of a plugin's component -
//! [`LoadedPlugin`] in the app, a fake in tests (split from `runtime.rs`).

use async_trait::async_trait;
use rb_plugin_host::LoadedPlugin;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

/// What the runtime asks of a plugin's component; [`LoadedPlugin`] in the app, a fake in tests.
#[async_trait]
pub trait Component: Send + Sync {
    /// A request of its page script in `view`.
    async fn page_message(&self, view: u32, message: &Value) -> Result<String, Failure>;
    /// A command of its manifest.
    async fn run_command(&self, command: &str, args: &str) -> Result<String, Failure>;
    /// A tool of its manifest.
    async fn call_tool(&self, tool: &str, args: &str) -> Result<String, Failure>;
    /// The host gave a page request up without an answer (review of RB-080-66, 5): counts as
    /// the plugin's failure, so a plugin that hangs is suspended in the end.
    fn gave_up(&self) {}
    /// Whether a call now would wait for a running one (RB-080-90).
    fn busy(&self) -> bool {
        false
    }
    /// Granted `consent-probe` (RB-094-09): its calls are offered the host's probe
    /// ([`super::consent`]).
    fn consent_probe(&self) -> bool {
        false
    }
    /// Granted `api-client` (RB-096-01): its calls are offered the host's API work
    /// (`crate::api_client::protocol`).
    fn api_client(&self) -> bool {
        false
    }
    /// Granted `load-test` (RB-096-07): its calls are offered the host's load test
    /// (`crate::load_test::protocol`).
    fn load_test(&self) -> bool {
        false
    }
    /// Its network rules (`@rules`, RB-070-02); only a plugin granted `network-rules` has any.
    async fn network_rules(&self, args: &str) -> Result<String, Failure> {
        let _ = args;
        Err(Failure::coded(
            FailureKind::NotSupported,
            codes::UNKNOWN_CONTRIBUTION,
        ))
    }
}

#[async_trait]
impl Component for LoadedPlugin {
    async fn page_message(&self, view: u32, message: &Value) -> Result<String, Failure> {
        LoadedPlugin::page_message(self, view, message).await
    }

    async fn run_command(&self, command: &str, args: &str) -> Result<String, Failure> {
        LoadedPlugin::run_command(self, command, args).await
    }

    async fn call_tool(&self, tool: &str, args: &str) -> Result<String, Failure> {
        LoadedPlugin::call_tool(self, tool, args).await
    }

    async fn network_rules(&self, args: &str) -> Result<String, Failure> {
        LoadedPlugin::network_rules(self, args).await
    }

    fn busy(&self) -> bool {
        LoadedPlugin::busy(self)
    }

    fn consent_probe(&self) -> bool {
        self.manifest()
            .grants
            .has(rb_plugin_types::Capability::ConsentProbe)
    }

    fn api_client(&self) -> bool {
        self.manifest()
            .grants
            .has(rb_plugin_types::Capability::ApiClient)
    }

    fn load_test(&self) -> bool {
        self.manifest()
            .grants
            .has(rb_plugin_types::Capability::LoadTest)
    }

    fn gave_up(&self) {
        if self.health().record_failure() {
            tracing::warn!(
                code = codes::SUSPENDED,
                plugin_id = self.manifest().id.as_str(),
                "a plugin's page requests went unanswered repeatedly; suspended until resumed"
            );
        }
    }
}
