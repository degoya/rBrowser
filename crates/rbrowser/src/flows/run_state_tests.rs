//! RB-094-08: a flow's `// @storageState <file>` loads that login state into the run's views'
//! sessions before the flow runs; a state that cannot be loaded stops the run before Node sees
//! it.

use std::sync::Mutex;

use serde_json::json;
use tokio::sync::{mpsc, watch};

use super::tests::{Fake, Project, passed, played};
use super::*;
use crate::cdp::lock;

/// [`Fake`] that records `load_state` and may refuse it.
#[derive(Default)]
struct StateFake {
    inner: Fake,
    loaded: Mutex<Vec<(String, Vec<u32>)>>,
    refuse: bool,
}

#[async_trait]
impl FlowDeps for StateFake {
    fn open_endpoints(
        &self,
        run: &[Runnable],
        targets: &[String],
    ) -> Result<Vec<Endpoint>, FlowError> {
        self.inner.open_endpoints(run, targets)
    }

    fn pause_sync(&self, views: &[ViewId]) -> Box<dyn Send + Sync> {
        self.inner.pause_sync(views)
    }

    async fn load_state(&self, name: &str, views: &[ViewId]) -> Result<(), FlowError> {
        lock(&self.loaded).push((name.to_owned(), views.iter().map(|v| v.0).collect()));
        if self.refuse {
            return Err(FlowError::Io(
                "the state file is no storageState".to_owned(),
            ));
        }
        Ok(())
    }

    async fn run_flow(
        &self,
        params: Value,
        timeout: Duration,
        progress: mpsc::UnboundedSender<Value>,
    ) -> Result<Value, FlowError> {
        self.inner.run_flow(params, timeout, progress).await
    }
}

fn request() -> RunRequest {
    RunRequest {
        name: "login".to_owned(),
        views: None,
        run_id: "20261120-101500-a1b2c3".to_owned(),
        inline: None,
    }
}

#[tokio::test]
async fn the_state_is_loaded_into_the_run_views_before_the_flow_runs() {
    let project = Project::new("// @storageState state/admin.json\nawait page.goto(env);\n");
    project.trust();
    let fake = StateFake::default();
    *lock(&fake.inner.answer) = Some(Ok(json!({ "views": [passed(1), passed(2)] })));
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    execute(&project.setup(), &request(), &fake, cancelled, progress)
        .await
        .expect("runs");
    assert_eq!(
        *lock(&fake.loaded),
        [("state/admin.json".to_owned(), vec![1, 2])]
    );
    assert_eq!(lock(&fake.inner.calls).len(), 1, "then the flow ran");
}

#[tokio::test]
async fn a_state_that_cannot_be_loaded_stops_the_run() {
    let project = Project::new("// @storageState state/admin.json\nawait page.goto(env);\n");
    project.trust();
    let fake = StateFake {
        refuse: true,
        ..StateFake::default()
    };
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    let outcome = execute(&project.setup(), &request(), &fake, cancelled, progress).await;
    assert!(matches!(outcome, Err(FlowError::Io(_))), "{outcome:?}");
    assert!(lock(&fake.inner.calls).is_empty(), "Node never saw the run");
}

#[tokio::test]
async fn a_flow_without_the_comment_loads_nothing() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = StateFake::default();
    *lock(&fake.inner.answer) = Some(Ok(json!({ "views": [passed(1), passed(2)] })));
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    execute(&project.setup(), &request(), &fake, cancelled, progress)
        .await
        .expect("runs");
    assert!(lock(&fake.loaded).is_empty());
}

#[tokio::test]
async fn a_played_recording_never_loads_a_state_its_text_names() {
    let project = Project::new("await page.goto(env);\n");
    project.trust();
    let fake = StateFake::default();
    *lock(&fake.inner.answer) = Some(Ok(json!({ "views": [passed(1)] })));
    let mut asked = played();
    if let Some(inline) = asked.inline.as_mut() {
        inline.source = "// @storageState state/admin.json\nawait page.goto(env);\n".to_owned();
    }
    let (_cancel, cancelled) = watch::channel(false);
    let (progress, _received) = mpsc::unbounded_channel();
    execute(&project.setup(), &asked, &fake, cancelled, progress)
        .await
        .expect("runs");
    assert!(
        lock(&fake.loaded).is_empty(),
        "only a flow file may load a state"
    );
    assert_eq!(
        lock(&fake.inner.calls).len(),
        1,
        "the recording still played"
    );
}
