//! "A password never appears in the log" (RB-020-05): a password's input and change run the
//! whole way - the agent's binding, the SyncHub, the CDP replay into the other views, which fail
//! here and are desynced - with every log line at TRACE written into a buffer; the buffer is
//! searched. The views get the value (the evaluations carry it); the log never does.

use std::io::Write;
use std::sync::{Arc, Mutex};

use rb_engine::{ViewHost, ViewId};
use serde_json::{Value, json};
use tracing_subscriber::fmt::MakeWriter;

use crate::cdp::lock;
use crate::replay::CdpReplay;
use crate::sync_hub::tests::{attach_all, emit_from, setup, until};
use crate::sync_hub::{for_host, on_start};

const PASSWORD: &str = "Tr0ub4dor&3-hunter2";

/// The log: every line any thread writes.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    fn text(&self) -> String {
        String::from_utf8_lossy(&lock(&self.0)).into_owned()
    }
}

impl Write for Log {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        lock(&self.0).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Log {
    type Writer = Log;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

fn target(path: &str) -> Value {
    json!({ "element": { "kind": "css", "path": path }, "offset": { "x": 0.5, "y": 0.5 } })
}

fn event(seq: u64, event: Value) -> Value {
    json!({ "kind": "event", "v": 1, "top": true, "seq": seq, "time": 0, "event": event })
}

/// nextest runs every test in a process of its own: the global subscriber is this test's.
#[tokio::test(flavor = "multi_thread")]
async fn a_password_never_appears_in_the_log() {
    let log = Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the test's own process");

    let setup = setup();
    on_start(&setup.core);
    let host = setup.core.host();
    let sync = for_host(host).expect("started");
    sync.set_replay(Arc::new(CdpReplay::for_host(host)));
    attach_all(&setup).await;
    host.set_active_view(ViewId(1)).expect("a view");
    until(|| sync.hub().with(|hub| hub.active()) == Some(ViewId(1))).await;

    let typed = json!({ "type": "input", "inputType": "insertText", "data": "2",
                        "value": PASSWORD, "sensitive": true, "target": target("#secret") });
    let changed = json!({ "type": "change", "value": PASSWORD, "sensitive": true,
                          "target": target("#secret") });
    emit_from(&setup, 1, &event(1, changed));
    emit_from(&setup, 1, &event(2, typed));

    // The fake browser answers no evaluation: the replays fail and the views are desynced.
    until(|| log.text().contains("replay failed")).await;
    until(|| log.text().matches("sync event").count() >= 2).await;
    let carried = setup
        .fake
        .sent("Runtime.evaluate")
        .iter()
        .any(|call| call.to_string().contains(PASSWORD));
    assert!(carried, "the other views get the value");
    let text = log.text();
    assert!(
        !text.contains(PASSWORD),
        "the password is in the log:\n{text}"
    );
    assert!(
        !text.contains("hunter2"),
        "a part of it is in the log:\n{text}"
    );
}
