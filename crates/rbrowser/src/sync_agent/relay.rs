//! The bus relay's delivery (RB-020-08): one queue and task per receiving view, so a page that
//! does not answer holds up neither the others nor the reports behind it (a `ready`, the next
//! message). A view's messages keep their order; a view [`QUEUE_DEPTH`] messages behind loses
//! the next ones.

use std::collections::HashMap;
use std::sync::{Arc, Weak};

use rb_engine::{ViewHost as _, ViewId};
use rb_sync::{BusMessage, BusPayload};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::host::Host;

/// Messages a view may fall behind before it loses the next ones.
pub const QUEUE_DEPTH: usize = 64;

/// One relayed message.
struct Delivery {
    from: ViewId,
    origin: String,
    r#type: String,
    payload: BusPayload,
}

/// The delivery queues, per receiving view.
pub struct BusRelay {
    runtime: Handle,
    queues: HashMap<ViewId, mpsc::Sender<Arc<Delivery>>>,
}

impl BusRelay {
    /// Queues that run on `runtime`.
    pub fn new(runtime: Handle) -> Self {
        Self {
            runtime,
            queues: HashMap::new(),
        }
    }

    /// Relays `message` of `from` to the other views when the core takes it, without waiting
    /// for any of them.
    pub fn relay(&mut self, host: &Arc<Host>, from: ViewId, message: &BusMessage) {
        let Some((targets, payload)) = super::bus::accepted_targets(host, from, message) else {
            return;
        };
        let views: Vec<ViewId> = host.views().iter().map(|info| info.id).collect();
        self.queues.retain(|view, _| views.contains(view));
        let delivery = Arc::new(Delivery {
            from,
            origin: message.origin.clone(),
            r#type: message.r#type.clone(),
            payload,
        });
        for target in targets {
            let runtime = &self.runtime;
            let sender = self
                .queues
                .entry(target)
                .or_insert_with(|| start(runtime, host, target));
            match sender.try_send(delivery.clone()) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    tracing::debug!(view = %target, "bus message not delivered: the view is behind");
                }
                // The task ended (the runtime is shutting down): a fresh one takes it.
                Err(mpsc::error::TrySendError::Closed(delivery)) => {
                    let fresh = start(runtime, host, target);
                    let _ = fresh.try_send(delivery);
                    self.queues.insert(target, fresh);
                }
            }
        }
    }
}

fn start(runtime: &Handle, host: &Arc<Host>, view: ViewId) -> mpsc::Sender<Arc<Delivery>> {
    let (sender, receiver) = mpsc::channel(QUEUE_DEPTH);
    runtime.spawn(deliver(Arc::downgrade(host), view, receiver));
    sender
}

/// Delivers `view`'s messages in order while the host lives.
async fn deliver(host: Weak<Host>, view: ViewId, mut queue: mpsc::Receiver<Arc<Delivery>>) {
    while let Some(delivery) = queue.recv().await {
        let Some(host) = host.upgrade() else {
            return;
        };
        host.page_api()
            .deliver(
                &[view],
                Some(delivery.from),
                Some(&delivery.origin),
                &delivery.r#type,
                &delivery.payload,
            )
            .await;
    }
}
