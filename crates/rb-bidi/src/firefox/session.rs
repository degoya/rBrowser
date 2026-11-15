//! [`Browser`]: one running Firefox and its BiDi session - launched, connected, version-checked,
//! subscribed to the events the engine follows - and its orderly end.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::broadcast;

use super::Launch;
use crate::browser::{self, MIN_FIREFOX_MAJOR, browser_version, check_version};
use crate::browsing_context::{self, CONTEXT_DESTROYED, NAVIGATION_EVENTS, top_contexts};
use crate::client::{BidiClient, DEFAULT_TIMEOUT, lock};
use crate::error::BidiError;
use crate::launcher::FirefoxProcess;
use crate::protocol::Event;
use crate::script::MESSAGE_EVENT;
use crate::storage::{create_user_context, is_default_session, user_context_of};

/// `session.new` may take a while on a cold start (1 to 1.5 s measured).
const SESSION_TIMEOUT: Duration = Duration::from_secs(30);
/// How long quitting waits for `browser.close`'s answer.
const CLOSE_GRACE: Duration = Duration::from_secs(1);
/// How long quitting then waits for the process to end before it is killed.
const EXIT_GRACE: Duration = Duration::from_secs(1);
/// The longest an orderly quit takes: the app's own wait for the external windows
/// (`engines::SHUTDOWN_WAIT` in `rbrowser`) must hold it, or the kill never comes.
pub const QUIT_BUDGET: Duration = CLOSE_GRACE.saturating_add(EXIT_GRACE);

/// A running Firefox.
pub(crate) struct Browser {
    /// The session.
    pub client: BidiClient,
    /// The version it reported.
    pub version: String,
    process: Mutex<Option<FirefoxProcess>>,
    /// The contexts of the window Firefox started with; closed once a view has its own.
    initial: Mutex<Vec<String>>,
    /// The user context of each named session (RB-040-05), made on its first use.
    user_contexts: tokio::sync::Mutex<HashMap<String, String>>,
}

impl Browser {
    /// Launches Firefox, opens the session and subscribes; answers the browser and its events
    /// (subscribed before the subscription, so none is missed).
    pub async fn start(
        launch: &dyn Launch,
    ) -> Result<(Browser, broadcast::Receiver<Event>), BidiError> {
        let launched = launch.launch().await?;
        // Dropping `launched.process` on an error below kills the process.
        let client = BidiClient::connect(&launched.endpoint, DEFAULT_TIMEOUT).await?;
        let events = client.events();
        let browser = Browser {
            client,
            version: String::new(),
            process: Mutex::new(launched.process),
            initial: Mutex::new(Vec::new()),
            user_contexts: tokio::sync::Mutex::new(HashMap::new()),
        };
        match browser.open_session().await {
            Ok((version, initial)) => {
                *lock(&browser.initial) = initial;
                Ok((Browser { version, ..browser }, events))
            }
            Err(error) => {
                browser.stop().await;
                Err(error)
            }
        }
    }

    async fn open_session(&self) -> Result<(String, Vec<String>), BidiError> {
        let answer = self
            .client
            .send_within(browser::session_new(), SESSION_TIMEOUT)
            .await?;
        let version = browser_version(&answer).unwrap_or_default();
        check_version(&version, MIN_FIREFOX_MAJOR)?;
        let mut events: Vec<&str> = NAVIGATION_EVENTS.to_vec();
        events.extend([CONTEXT_DESTROYED, MESSAGE_EVENT]);
        self.client.send(browser::subscribe(&events)).await?;
        let tree = self.client.send(browsing_context::get_tree(None)).await?;
        Ok((version, top_contexts(&tree)))
    }

    /// The user context of `session`: `None` (the default one) for the default session; a
    /// named session's is made on its first use and kept while this Firefox runs. A Firefox
    /// that cannot make one keeps the session in the default context and says so in the log.
    pub async fn user_context(&self, session: &str) -> Result<Option<String>, BidiError> {
        if is_default_session(session) {
            return Ok(None);
        }
        let mut known = self.user_contexts.lock().await;
        if let Some(id) = known.get(session) {
            return Ok(Some(id.clone()));
        }
        match self.client.send(create_user_context()).await {
            Ok(answer) => {
                let id = user_context_of(&answer)?;
                known.insert(session.to_owned(), id.clone());
                Ok(Some(id))
            }
            Err(error) if error.is_unsupported() => {
                tracing::warn!(%session, "Firefox has no user contexts: the session shares the default one");
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// The process id of the Firefox rBrowser started, while it runs; `None` for a Firefox
    /// started elsewhere (a test's fake).
    pub fn process_id(&self) -> Option<u32> {
        lock(&self.process).as_ref().and_then(FirefoxProcess::id)
    }

    /// Closes the start window's contexts (once): a view's own window stands now.
    pub async fn close_initial(&self) {
        let initial = std::mem::take(&mut *lock(&self.initial));
        for context in initial {
            if let Err(error) = self.client.send(browsing_context::close(&context)).await {
                tracing::debug!(%error, "the start window did not close");
            }
        }
    }

    /// After the connection dropped: whether Firefox ended by itself cleanly - it quits with
    /// its last window, which is the person's hand, not a crash (RB-080-38). A process that
    /// is gone already (an orderly `stop`), still running or ended with an error counts as
    /// not clean.
    pub async fn ended_cleanly(&self) -> bool {
        let process = lock(&self.process).take();
        match process {
            Some(process) => process.ended_cleanly(EXIT_GRACE).await == Some(true),
            None => false,
        }
    }

    /// Quits Firefox: `browser.close`, then the process after a grace.
    pub async fn stop(&self) {
        if !self.client.is_closed()
            && let Err(error) = self.client.send_within(browser::close(), CLOSE_GRACE).await
        {
            tracing::debug!(%error, "browser.close");
        }
        self.client.close();
        let process = lock(&self.process).take();
        if let Some(process) = process {
            process.stop(EXIT_GRACE).await;
        }
    }
}
