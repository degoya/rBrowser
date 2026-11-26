//! The questions of rBrowser's sign-in dialog (RB-099-04), without CEF: one question per origin,
//! realm and proxy, whichever views wait on it. Every request waiting (a `W`, CEF's callback in
//! the app) joins the open question of its key and gets its one answer; the question is
//! withdrawn when its last view closes (at the end of the app every view closes), and a closed
//! view's late request is never asked. Only refusals count: the same challenge again in a view
//! within [`REFUSAL_WINDOW`] of an answer means the server refused it, so the question says so,
//! and after [`MAX_TRIES`] refused answers in a row the request is cancelled (the server's 401
//! page) instead of asking in a loop. A challenge later than that is a fresh question - the
//! answer before it was accepted. Setting or removing a credential starts over.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use rb_engine::ViewId;
use rb_ipc::AuthPrompt;

/// The refused answers in a row per view and question before the view is no longer asked.
pub const MAX_TRIES: u32 = 3;

/// How soon after an answer the same challenge means the server refused it: Chromium sends the
/// answer at once and asks again at once when it is refused.
pub const REFUSAL_WINDOW: Duration = Duration::from_secs(30);

/// What makes one question: the origin (or the proxy's host and port), the realm, a proxy.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PromptKey {
    /// `scheme://host:port` of the server, `host:port` of a proxy.
    pub origin: String,
    /// The realm; empty when the server names none.
    pub realm: String,
    /// Whether a proxy asks.
    pub proxy: bool,
}

/// A request that waits for the person.
pub struct Waiting<W> {
    /// The question it waits on.
    pub key: PromptKey,
    /// Its view.
    pub view: ViewId,
    /// Its CEF browser.
    pub browser: i32,
    /// The environment whose credential may be stored from the answer.
    pub environment: Option<String>,
    /// The server refused the stored credential just now.
    pub stored_refused: bool,
    /// Whether the answer would cross the network unencrypted (plain `http`, not a proxy).
    pub insecure: bool,
    /// What gets the answer.
    pub waiter: W,
}

/// What [`Prompts::ask`] decided.
pub enum Asked<W> {
    /// Show `prompt`: a `new` question, or an open one with one more view.
    Show {
        /// The question.
        prompt: AuthPrompt,
        /// Whether it was not open before.
        new: bool,
    },
    /// The server refused [`MAX_TRIES`] answers in a row in this view: the request is cancelled.
    Cancel(W),
    /// The view closed before its request was asked: the request is cancelled.
    Gone(W),
}

/// What the shell is told after views closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The question with fewer views.
    Show(AuthPrompt),
    /// The question `id` is withdrawn.
    Closed(u64),
}

/// The question answered.
#[derive(Debug)]
pub struct Answered<W> {
    /// Its key.
    pub key: PromptKey,
    /// Every request that waited on it, in order.
    pub waiters: Vec<W>,
}

struct Open<W> {
    id: u64,
    key: PromptKey,
    environment: Option<String>,
    refused: bool,
    insecure: bool,
    waiting: Vec<(ViewId, i32, W)>,
}

impl<W> Open<W> {
    fn prompt(&self) -> AuthPrompt {
        let mut views: Vec<ViewId> = Vec::new();
        for (view, _, _) in &self.waiting {
            if !views.contains(view) {
                views.push(*view);
            }
        }
        AuthPrompt {
            id: self.id,
            origin: self.key.origin.clone(),
            realm: self.key.realm.clone(),
            proxy: self.key.proxy,
            views,
            environment: self.environment.clone(),
            refused: self.refused,
            insecure: self.insecure,
        }
    }

    fn waiters(self) -> Vec<W> {
        self.waiting
            .into_iter()
            .map(|(_, _, waiter)| waiter)
            .collect()
    }
}

/// The open questions, the answers given per view - how many in a row and when the last - and
/// the views closed.
pub struct Prompts<W> {
    last_id: u64,
    open: Vec<Open<W>>,
    answers: HashMap<(i32, PromptKey), (u32, Instant)>,
    closed: HashSet<i32>,
}

impl<W> Default for Prompts<W> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W> Prompts<W> {
    /// No question open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            last_id: 0,
            open: Vec::new(),
            answers: HashMap::new(),
            closed: HashSet::new(),
        }
    }

    /// Whether no question is open.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.open.is_empty()
    }

    /// `waiting` joins the open question of its key, or opens one; now.
    pub fn ask(&mut self, waiting: Waiting<W>) -> Asked<W> {
        self.ask_at(waiting, Instant::now())
    }

    /// `waiting` joins the open question of its key, or opens one, at `now`.
    pub fn ask_at(&mut self, waiting: Waiting<W>, now: Instant) -> Asked<W> {
        if self.closed.contains(&waiting.browser) {
            return Asked::Gone(waiting.waiter);
        }
        self.forget_before(now);
        let refusals = self
            .answers
            .get(&(waiting.browser, waiting.key.clone()))
            .map_or(0, |(count, _)| *count);
        if refusals >= MAX_TRIES {
            return Asked::Cancel(waiting.waiter);
        }
        let refused = refusals > 0 || waiting.stored_refused;
        let entry = (waiting.view, waiting.browser, waiting.waiter);
        if let Some(open) = self.open.iter_mut().find(|open| open.key == waiting.key) {
            open.refused |= refused;
            if open.environment.is_none() {
                open.environment = waiting.environment;
            }
            open.waiting.push(entry);
            return Asked::Show {
                prompt: open.prompt(),
                new: false,
            };
        }
        self.last_id += 1;
        let open = Open {
            id: self.last_id,
            key: waiting.key,
            environment: waiting.environment,
            refused,
            insecure: waiting.insecure,
            waiting: vec![entry],
        };
        let prompt = open.prompt();
        self.open.push(open);
        Asked::Show { prompt, new: true }
    }

    /// The environment the open question `id` may store a credential for; `None` when no
    /// question `id` is open.
    #[must_use]
    pub fn environment(&self, id: u64) -> Option<Option<String>> {
        self.open
            .iter()
            .find(|open| open.id == id)
            .map(|open| open.environment.clone())
    }

    /// The origin the open question `id` asks for, as shown; `None` when it is not open.
    #[must_use]
    pub fn origin(&self, id: u64) -> Option<String> {
        self.open
            .iter()
            .find(|open| open.id == id)
            .map(|open| open.key.origin.clone())
    }

    /// The person answered the question `id` now: every request waiting on it.
    pub fn sign_in(&mut self, id: u64) -> Option<Answered<W>> {
        self.sign_in_at(id, Instant::now())
    }

    /// The person answered the question `id` at `now`: every request waiting on it, the answer
    /// counted per view - one more in a row while the server keeps refusing, else the first.
    pub fn sign_in_at(&mut self, id: u64, now: Instant) -> Option<Answered<W>> {
        self.forget_before(now);
        let open = self.take(id)?;
        for (_, browser, _) in &open.waiting {
            let entry = self
                .answers
                .entry((*browser, open.key.clone()))
                .or_insert((0, now));
            *entry = (entry.0 + 1, now);
        }
        let key = open.key.clone();
        Some(Answered {
            key,
            waiters: open.waiters(),
        })
    }

    /// The person cancelled the question `id`, or it waited too long: every request waiting on
    /// it.
    pub fn cancel(&mut self, id: u64) -> Option<Vec<W>> {
        self.take(id).map(Open::waiters)
    }

    /// The view of `browser` closed: its requests, and what the shell is told. A request of it
    /// that comes later (its store read still ran) is never asked.
    pub fn drop_browser(&mut self, browser: i32) -> (Vec<W>, Vec<Change>) {
        self.closed.insert(browser);
        self.answers.retain(|(asked, _), _| *asked != browser);
        let mut dropped = Vec::new();
        let mut changes = Vec::new();
        for open in &mut self.open {
            let before = open.waiting.len();
            let (gone, kept): (Vec<_>, Vec<_>) = std::mem::take(&mut open.waiting)
                .into_iter()
                .partition(|(_, asked, _)| *asked == browser);
            open.waiting = kept;
            if open.waiting.len() == before {
                continue;
            }
            dropped.extend(gone.into_iter().map(|(_, _, waiter)| waiter));
            changes.push(if open.waiting.is_empty() {
                Change::Closed(open.id)
            } else {
                Change::Show(open.prompt())
            });
        }
        self.open.retain(|open| !open.waiting.is_empty());
        (dropped, changes)
    }

    /// A credential was set or removed: every view may be answered again.
    pub fn reset_tries(&mut self) {
        self.answers.clear();
    }

    /// An answer older than [`REFUSAL_WINDOW`] was accepted: its view starts over.
    fn forget_before(&mut self, now: Instant) {
        self.answers
            .retain(|_, (_, at)| now.saturating_duration_since(*at) <= REFUSAL_WINDOW);
    }

    fn take(&mut self, id: u64) -> Option<Open<W>> {
        let index = self.open.iter().position(|open| open.id == id)?;
        Some(self.open.remove(index))
    }
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
