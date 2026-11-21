//! The window's prober (RB-094-09): one consent probe at a time over [`super::cdp::CdpDriver`],
//! its address held to [`super::address`] first (`plugins.error.probe_url_denied` naming the
//! setting `plugins.consentProbeAnyUrl`), its failures named by a fixed sentence (split from
//! `cdp.rs`).

use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use rb_plugin_types::{Failure, FailureKind, codes};
use serde_json::Value;

use super::address::Admission;
use super::args::ProbeRequest;
use super::call::Prober;
use super::cdp::CdpDriver;
use super::run::probe;
use crate::control::Core;
use crate::plugins::services::local_origins;

/// The window's probes: one at a time.
pub struct HostProber {
    core: Weak<Core>,
    one: tokio::sync::Mutex<()>,
}

impl HostProber {
    #[must_use]
    pub fn new(core: &Arc<Core>) -> Self {
        Self {
            core: Arc::downgrade(core),
            one: tokio::sync::Mutex::new(()),
        }
    }
}

#[async_trait]
impl Prober for HostProber {
    async fn probe(&self, request: &ProbeRequest) -> Result<Value, Failure> {
        let _one = self.one.lock().await;
        let unavailable = || {
            Failure::coded(FailureKind::NotSupported, codes::NOT_AVAILABLE)
                .with_param("service", "consent probe")
        };
        let core = self.core.upgrade().ok_or_else(unavailable)?;
        let host = Arc::clone(core.host());
        let admission = Admission {
            any: core.user_settings().plugins.consent_probe_any_url,
            own: local_origins(&host.config(), host.shell_state().trusted),
        };
        drop(core);
        if admission.check(&request.url).await.is_err() {
            return Err(
                Failure::coded(FailureKind::AccessDenied, codes::PROBE_URL_DENIED)
                    .with_param("url", &request.url),
            );
        }
        let mut driver = CdpDriver::new(host.cdp().clone(), Arc::downgrade(&host), admission);
        drop(host);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        match probe(&mut driver, request, now).await {
            Ok(capture) => {
                if !capture.discarded {
                    tracing::warn!("the consent probe's session was not confirmed discarded");
                }
                Ok(capture.to_value())
            }
            Err(error) => {
                // A fixed sentence of the host's: no address, no page text.
                tracing::warn!(reason = %error, "the consent probe failed");
                Err(Failure::coded(FailureKind::Other, codes::HOST_FAILED)
                    .with_param("service", "consent probe")
                    .with_param("reason", error.0))
            }
        }
    }
}
