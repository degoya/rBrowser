//! The engine component rBrowser ships (`engine/rb-script-engine.wasm`) in the sandbox: it loads
//! with no import but the host, runs a script, reaches the tools only through the host, has no
//! network, file system or process, gets its randomness and clock, and an endless loop ends at
//! the deadline.

use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use rb_script::{Limits, RunError, Sandbox, Tools};

/// One sandbox for the file: the component is large and compiles once.
fn sandbox() -> &'static Sandbox {
    static SANDBOX: OnceLock<Sandbox> = OnceLock::new();
    SANDBOX.get_or_init(|| Sandbox::new().expect("the shipped engine loads"))
}

const LIMITS: Limits = Limits {
    timeout: Duration::from_secs(20),
    memory_bytes: 256 << 20,
};

#[derive(Default)]
struct Echo {
    calls: Mutex<Vec<String>>,
}

#[async_trait::async_trait]
impl Tools for Echo {
    async fn call(&self, tool: String, arguments: String) -> Result<String, String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tool.clone());
        if tool == "evaluate" {
            return Err(r#"{"code":"evaluate_disabled","message":"off"}"#.to_owned());
        }
        Ok(format!(r#"{{"tool":"{tool}","arguments":{arguments}}}"#))
    }
}

async fn run(code: &str, tools: Arc<Echo>) -> Result<String, RunError> {
    sandbox().run(code, "{}", "\"all\"", LIMITS, tools).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_script_runs_and_reaches_its_tools_through_the_host_only() {
    let tools = Arc::new(Echo::default());
    let answer = run(
        "const page = await rb.views_list({ target });\n\
         return { page, kinds: [typeof fetch, typeof require, typeof process],\n\
                  random: typeof Math.random(), clock: Date.now() > 1.6e12 };",
        tools.clone(),
    )
    .await
    .expect("the script answers");
    let answer: serde_json::Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(answer["value"]["page"]["tool"], "views_list");
    assert_eq!(answer["value"]["page"]["arguments"]["target"], "all");
    assert_eq!(
        answer["value"]["kinds"],
        serde_json::json!(["undefined", "undefined", "undefined"])
    );
    assert_eq!(answer["value"]["random"], "number");
    assert_eq!(answer["value"]["clock"], true);
    let calls = tools.calls.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(*calls, ["views_list"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_refusal_comes_back_as_the_scripts_failure() {
    let Err(RunError::Script(failure)) =
        run("await rb.evaluate({ expression: '1' });", Arc::default()).await
    else {
        panic!("the refusal fails the script");
    };
    let failure: serde_json::Value = serde_json::from_str(&failure).expect("JSON");
    assert_eq!(failure["code"], "evaluate_disabled");
    assert_eq!(failure["tool"], "evaluate");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_endless_loop_ends_at_the_deadline() {
    let limits = Limits {
        timeout: Duration::from_millis(300),
        ..LIMITS
    };
    let answer = sandbox()
        .run(
            "while (true) {}",
            "{}",
            "\"all\"",
            limits,
            Arc::new(Echo::default()),
        )
        .await;
    assert_eq!(answer, Err(RunError::Timeout));
}
