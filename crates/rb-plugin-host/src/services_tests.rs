use std::sync::{Arc, Mutex};

use super::{CallSpan, HostServices};

/// Records what the services hear of the calls, in order.
#[derive(Default)]
struct Heard(Mutex<Vec<String>>);

impl Heard {
    fn lines(&self) -> Vec<String> {
        self.0.lock().map(|lines| lines.clone()).unwrap_or_default()
    }
}

impl HostServices for Heard {
    fn call_started(&self, plugin_id: &str) {
        if let Ok(mut lines) = self.0.lock() {
            lines.push(format!("started {plugin_id}"));
        }
    }

    fn call_ended(&self, plugin_id: &str) {
        if let Ok(mut lines) = self.0.lock() {
            lines.push(format!("ended {plugin_id}"));
        }
    }
}

#[test]
fn a_call_is_started_when_made_and_ended_when_dropped() {
    let heard = Arc::new(Heard::default());
    let services: Arc<dyn HostServices> = heard.clone();
    let span = CallSpan::start(&services, "width-sweep");
    assert_eq!(heard.lines(), ["started width-sweep"]);
    drop(span);
    assert_eq!(heard.lines(), ["started width-sweep", "ended width-sweep"]);
}

#[tokio::test]
async fn a_call_whose_caller_stopped_waiting_ends_too() {
    let heard = Arc::new(Heard::default());
    let services: Arc<dyn HostServices> = heard.clone();
    let call = async move {
        let _span = CallSpan::start(&services, "width-sweep");
        std::future::pending::<()>().await;
    };
    // A cancelled check run drops the job's future while the plugin waits in a host call.
    let stopped = tokio::time::timeout(std::time::Duration::from_millis(10), call).await;
    assert!(stopped.is_err());
    assert_eq!(heard.lines(), ["started width-sweep", "ended width-sweep"]);
}

#[test]
fn the_default_services_hear_nothing() {
    let services: Arc<dyn HostServices> = Arc::new(super::NoServices);
    drop(CallSpan::start(&services, "any"));
}

/// RB-090-32: a span's wait and run are the counters `plugin.wait` and `plugin.call`.
#[test]
fn a_span_counts_its_wait_and_its_run() {
    use rb_util::perf::{global, names};
    let count = |name: &str| {
        global()
            .snapshot()
            .iter()
            .find(|series| series.name == name)
            .map_or(0, |series| series.count)
    };
    let (waits, calls) = (count(names::PLUGIN_WAIT), count(names::PLUGIN_CALL));
    let services: Arc<dyn HostServices> = Arc::new(super::NoServices);
    let span = CallSpan::after_wait(&services, "any", std::time::Instant::now());
    assert!(count(names::PLUGIN_WAIT) > waits);
    drop(span);
    assert!(count(names::PLUGIN_CALL) > calls);
}
