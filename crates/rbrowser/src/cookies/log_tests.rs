//! "A log scan finds no cookie value" (RB-040-05): a copy runs the whole way into a real
//! Firefox engine (over a fake Firefox) and a real WebKit engine (over a fake sidecar), once
//! taken and once refused, with every log line at TRACE written into a buffer; the buffer and
//! the report are searched for the value.

use std::io::Write;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rb_bidi::FirefoxEngine;
use rb_bidi::storage::Cookie as BidiCookie;
use rb_bidi::testing::FakeFirefox;
use rb_engine::ViewId;
use tracing_subscriber::fmt::MakeWriter;

use super::copy::{Jars, Options, Taken, run};
use super::jars::tests::{firefox, sid, webkit};
use super::jars::{into_firefox, into_webkit};
use super::mapping::PlaywrightCookie;
use super::model::ChromeCookie;
use super::plan::{Group, Plan};
use crate::cdp::lock;
use crate::engines::webkit::WebkitEngine;
use crate::engines::webkit::fake::FakeSidecar;

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

struct EngineJars {
    firefox: Arc<FirefoxEngine>,
    webkit: Arc<WebkitEngine>,
}

#[async_trait]
impl Jars for EngineJars {
    async fn read(&self, _view: ViewId) -> Result<Vec<ChromeCookie>, String> {
        Ok(vec![sid()])
    }

    async fn set_firefox(&self, session: &str, cookies: &[BidiCookie]) -> Result<Taken, String> {
        into_firefox(Some(self.firefox.clone()), session, cookies).await
    }

    async fn set_webkit(
        &self,
        view: ViewId,
        cookies: &[PlaywrightCookie],
    ) -> Result<Taken, String> {
        into_webkit(Some(self.webkit.clone()), view, cookies).await
    }

    async fn reload(&self, _view: ViewId) -> Result<(), String> {
        Ok(())
    }
}

fn plan() -> Plan {
    Plan {
        groups: vec![Group {
            session: "admin".to_owned(),
            readers: vec![ViewId(1)],
            firefox: vec![ViewId(5)],
            webkit: vec![ViewId(6), ViewId(7)],
            hosts: vec!["127.0.0.1".to_owned()],
        }],
        other_session: Vec::new(),
    }
}

/// nextest runs every test in a process of its own: the global subscriber is this test's.
#[tokio::test(flavor = "multi_thread")]
async fn a_cookie_value_never_appears_in_the_log_or_the_report() {
    let log = Log::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("the test's own process");

    let fake_firefox = FakeFirefox::start().await;
    let fake_sidecar = FakeSidecar::new();
    let jars = EngineJars {
        firefox: firefox(&fake_firefox).await,
        webkit: webkit(&fake_sidecar).await,
    };
    let options = Options {
        domains: None,
        all_domains: false,
        environment_host: None,
        exclude: Vec::new(),
        reload: true,
        now: 1_800_000_000,
    };
    let taken = run(&jars, plan(), &options).await;
    assert_eq!(taken.copied, 2, "{taken:?}");
    fake_firefox.refuse("storage.setCookie", "unable to set cookie");
    fake_sidecar.fail_next(
        crate::engines::webkit::rpc::COOKIES,
        -32602,
        "cookies[0].value must be a string",
    );
    let refused = run(&jars, plan(), &options).await;
    assert_eq!(refused.views[0].refused, 1, "{refused:?}");
    assert!(refused.failed() >= 1, "{refused:?}");

    let carried = fake_firefox.calls("storage.setCookie");
    assert!(
        carried[0].to_string().contains("hunter2"),
        "Firefox gets the value"
    );
    let text = log.text();
    assert!(!text.is_empty(), "something was logged");
    for shown in [
        text,
        serde_json::to_string(&taken).expect("json"),
        serde_json::to_string(&refused).expect("json"),
        format!("{taken:?} {refused:?}"),
    ] {
        assert!(!shown.contains("hunter2"), "{shown}");
    }
}
