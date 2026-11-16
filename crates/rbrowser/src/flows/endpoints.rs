//! The internal CDP endpoints of a run: one proxy per view ([`super::scope`]), on a free port of
//! 127.0.0.1, each with a fresh token that the log's mask knows. They live as long as the run:
//! dropping an [`Endpoint`]'s guard stops its proxy; its slot is freed and its token leaves the
//! mask once the proxy's last connection ended. The user's own proxy (`cdp.enabled`) is not
//! involved, so flows run with it switched off.

use std::sync::Arc;

use rb_cdp::{ProxyToken, TargetFilter};
use serde_json::json;
use tokio::runtime::Handle;

use super::FlowError;
use super::plan::Runnable;
use super::run::Endpoint;
use super::scope::claim;
use crate::cdp::CdpRouter;
use crate::cdp_proxy::endpoint::http_url;
use crate::cdp_proxy::{Hooks, ProxyServer};

/// A running proxy: stops it. Its slot is the proxy's to keep ([`ProxyServer::keep_until_dropped`])
/// and is freed once its last connection ended - a run that starts at once cannot get a slot
/// an old connection still judges targets with.
struct Guard {
    server: Arc<ProxyServer>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.server.stop();
    }
}

/// A run's token, masked in the log while its proxy or a connection of it lives and forgotten
/// after, so the mask does not grow with every run.
struct Unmask(String);

impl Drop for Unmask {
    fn drop(&mut self) {
        rb_diag::unregister_secret(&self.0);
    }
}

/// Opens an endpoint for each view of `run`; `targets` are every view's page, `hooks` makes the
/// proxy's hooks around a view's filter.
///
/// # Errors
///
/// [`FlowError::Proxy`] when no slot is free, no token can be made or a proxy cannot listen;
/// the endpoints opened so far are closed again.
pub fn open(
    runtime: &Handle,
    router: &Arc<CdpRouter>,
    hooks: &dyn Fn(&'static dyn TargetFilter) -> Hooks,
    run: &[Runnable],
    targets: &[String],
) -> Result<Vec<Endpoint>, FlowError> {
    run.iter()
        .map(|runnable| {
            let claimed = claim(&runnable.target, targets).ok_or_else(|| {
                FlowError::Proxy(format!(
                    "more than {} views run flows at once",
                    super::scope::SLOTS
                ))
            })?;
            let token =
                ProxyToken::generate().map_err(|error| FlowError::Proxy(error.to_string()))?;
            rb_diag::register_secret(token.expose());
            let server = ProxyServer::start(
                runtime,
                Arc::clone(router),
                None,
                token.clone(),
                hooks(claimed.filter()),
            )
            .map_err(|error| FlowError::Proxy(error.to_string()))?;
            server.keep_until_dropped(Box::new(claimed));
            server.keep_until_dropped(Box::new(Unmask(token.expose().to_owned())));
            let value = json!({
                "url": http_url(server.port()),
                "headers": { "Authorization": token.bearer() },
            });
            Ok(Endpoint {
                value,
                guard: Box::new(Guard { server }),
            })
        })
        .collect()
}
