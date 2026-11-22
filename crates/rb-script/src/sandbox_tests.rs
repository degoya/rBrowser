use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use super::*;
use crate::testbed::{self, Bed};

/// Records every call and answers `answer`, after `delay`.
#[derive(Default)]
struct Recording {
    calls: Mutex<Vec<(String, String)>>,
    answer: Option<Result<String, String>>,
    delay: Option<Duration>,
}

#[async_trait::async_trait]
impl Tools for Recording {
    async fn call(&self, tool: String, arguments: String) -> Result<String, String> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((tool, arguments));
        if let Some(delay) = self.delay {
            tokio::time::sleep(delay).await;
        }
        self.answer.clone().unwrap_or_else(|| Ok("{}".to_owned()))
    }
}

fn limits(timeout_ms: u64) -> Limits {
    Limits {
        timeout: Duration::from_millis(timeout_ms),
        memory_bytes: 4 << 20,
    }
}

async fn run(bed: &Bed, limits: Limits, tools: Arc<Recording>) -> Result<String, RunError> {
    let sandbox = Sandbox::from_component(&bed.build()).expect("the bed loads");
    sandbox
        .run("return 1;", "{}", "\"all\"", limits, tools)
        .await
}

#[tokio::test]
async fn the_engines_answer_and_failure_pass_through() {
    let tools = Arc::new(Recording::default());
    let answer = run(
        &Bed::answering(r#"{"value":1}"#),
        limits(5000),
        tools.clone(),
    )
    .await;
    assert_eq!(answer, Ok(r#"{"value":1}"#.to_owned()));
    let failure = run(
        &Bed::failing(r#"{"code":"script.syntax"}"#),
        limits(5000),
        tools,
    )
    .await;
    assert_eq!(
        failure,
        Err(RunError::Script(r#"{"code":"script.syntax"}"#.to_owned()))
    );
}

#[tokio::test]
async fn an_endless_script_is_stopped_at_its_deadline() {
    let answer = run(&Bed::endless(), limits(100), Arc::default()).await;
    assert_eq!(answer, Err(RunError::Timeout));
}

#[tokio::test]
async fn a_stack_overflow_and_a_crash_of_the_engine_are_the_scripts_failures() {
    let Err(RunError::Script(deep)) = run(&Bed::recursing(), limits(5000), Arc::default()).await
    else {
        panic!("a stack overflow fails the script");
    };
    assert!(deep.contains(r#""code":"script.stack""#), "{deep}");
    let Err(RunError::Script(crash)) = run(&Bed::crashing(), limits(5000), Arc::default()).await
    else {
        panic!("a crash fails the script");
    };
    assert!(crash.contains(r#""code":"script.error""#), "{crash}");
}

#[tokio::test]
async fn a_script_that_takes_more_memory_than_its_limit_is_stopped() {
    let answer = run(&Bed::growing(), limits(5000), Arc::default()).await;
    assert_eq!(answer, Err(RunError::OutOfMemory));
}

#[tokio::test]
async fn every_tool_call_reaches_the_callers_tools_and_its_answer_the_script() {
    let tools = Arc::new(Recording {
        answer: Some(Ok(r#"{"url":"https://a.test/"}"#.to_owned())),
        ..Recording::default()
    });
    let answer = run(
        &Bed::calling("navigate", r#"{"target":"all"}"#, 3),
        limits(5000),
        tools.clone(),
    )
    .await;
    assert_eq!(answer, Ok(r#"{"url":"https://a.test/"}"#.to_owned()));
    let calls = tools.calls.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(calls.len(), 3);
    assert_eq!(
        calls[0],
        ("navigate".to_owned(), r#"{"target":"all"}"#.to_owned())
    );
}

#[tokio::test]
async fn a_refused_call_reaches_the_script_as_its_error() {
    let tools = Arc::new(Recording {
        answer: Some(Err(r#"{"code":"script.recursion"}"#.to_owned())),
        ..Recording::default()
    });
    let answer = run(&Bed::calling("script_run", "{}", 1), limits(5000), tools).await;
    assert_eq!(
        answer,
        Err(RunError::Script(
            r#"{"code":"script.recursion"}"#.to_owned()
        ))
    );
}

#[tokio::test]
async fn a_tool_call_that_outlasts_the_deadline_ends_the_run() {
    let tools = Arc::new(Recording {
        delay: Some(Duration::from_secs(3600)),
        ..Recording::default()
    });
    let answer = run(&Bed::calling("wait_for", "{}", 1), limits(100), tools).await;
    assert_eq!(answer, Err(RunError::Timeout));
}

#[tokio::test]
async fn nothing_survives_between_runs() {
    let sandbox = Sandbox::from_component(&Bed::counting().build()).expect("the bed loads");
    for _ in 0..2 {
        let answer = sandbox
            .run(
                "",
                "{}",
                "\"all\"",
                limits(5000),
                Arc::new(Recording::default()),
            )
            .await;
        assert_eq!(answer, Ok("1".to_owned()), "a fresh instance every run");
    }
}

#[test]
fn a_component_importing_anything_but_the_host_is_refused() {
    let Err(error) = Sandbox::from_component(&testbed::importing_wasi()) else {
        panic!("a WASI import is refused");
    };
    assert!(error.to_string().contains("wasi:filesystem"), "{error}");
}
