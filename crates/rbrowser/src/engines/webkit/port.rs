//! What the WebKit engine needs of the Node sidecar: requests, requests that never start it, and
//! its notifications. [`SidecarPort`] is implemented by the app's one `rb_sidecar::Sidecar` and,
//! in tests, by a fake that plays the sidecar's `webkit` module.

use std::sync::Arc;

use async_trait::async_trait;
use rb_sidecar::rpc::Notification;
use rb_sidecar::{Launcher, Sidecar, SidecarError, Subscription};
use serde_json::Value;

/// The notifications of one sidecar process; `None` once it ended.
#[async_trait]
pub trait Notifications: Send {
    /// The next notification.
    async fn recv(&mut self) -> Option<Notification>;
}

/// The sidecar as the WebKit engine uses it.
#[async_trait]
pub trait SidecarPort: Send + Sync {
    /// A request; starts the sidecar when none runs.
    async fn request(&self, method: &str, params: Value) -> Result<Value, SidecarError>;

    /// A request only when a sidecar runs (closing windows never starts one).
    async fn request_if_running(
        &self,
        method: &str,
        params: Value,
    ) -> Option<Result<Value, SidecarError>>;

    /// The running sidecar's notifications from now on; starts it when none runs.
    async fn subscribe(&self) -> Result<Box<dyn Notifications>, SidecarError>;
}

#[async_trait]
impl Notifications for Subscription {
    async fn recv(&mut self) -> Option<Notification> {
        Subscription::recv(self).await
    }
}

/// The app's sidecar (`crate::sidecar`), or one of a test's own launcher.
pub struct AppSidecar<L>(pub Arc<Sidecar<L>>);

#[async_trait]
impl<L: Launcher + 'static> SidecarPort for AppSidecar<L> {
    async fn request(&self, method: &str, params: Value) -> Result<Value, SidecarError> {
        self.0.request(method, params).await
    }

    async fn request_if_running(
        &self,
        method: &str,
        params: Value,
    ) -> Option<Result<Value, SidecarError>> {
        self.0.request_if_running(method, params).await
    }

    async fn subscribe(&self) -> Result<Box<dyn Notifications>, SidecarError> {
        let subscription = self.0.subscribe().await?;
        Ok(Box::new(subscription))
    }
}
