//! Forcing and resetting over CDP with the model (RB-090-53), free of the window: the views come
//! as data ([`ViewPage`]), the element of the clicked view already resolved ([`Source`]). The
//! registry is locked only between the calls, never across one.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rb_engine::{EngineError, ViewId};
use serde_json::Value;

use super::cdp;
use super::model::{Change, PseudoClass, Registry};
use crate::cdp::{CdpRouter, lock};

/// A view to force in: its page session (none for a view without CDP - Firefox, WebKit) and its
/// sync agent's session and world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewPage {
    pub view: ViewId,
    pub page: Option<String>,
    pub agent: Option<(String, i64)>,
}

/// The element the request starts from: in `view`, resolved at the point or picked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub view: ViewId,
    pub backend: i64,
    /// The view's sync agent, which names the element for the other views.
    pub agent: Option<(String, i64)>,
}

/// Which element in each view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Element {
    /// The source's element there, the same one in the others (the sync agent's selectors).
    From(Source),
    /// The first match of a CSS selector in each view.
    Selector(String),
}

/// What a request does to the element's states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// Exactly these; none takes the element's states back.
    Set(BTreeSet<PseudoClass>),
    /// The menu's switch: by what the source's element has.
    Toggle(PseudoClass),
}

/// What became of one view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// The element has these states now.
    Forced(BTreeSet<PseudoClass>),
    /// The view has no such element.
    Missing,
    /// Something failed; why.
    Failed(String),
}

/// Forces `request` on `element` in each of `views`; one outcome per view, in order.
pub async fn force(
    cdp: &Arc<CdpRouter>,
    registry: &Mutex<Registry>,
    views: &[ViewPage],
    element: &Element,
    request: &Request,
) -> Vec<(ViewId, Outcome)> {
    let change = match request {
        Request::Set(classes) => Change::Set(classes.clone()),
        Request::Toggle(class) => {
            let had = match element {
                Element::From(source) => lock(registry).classes_of(source.view, source.backend),
                Element::Selector(_) => BTreeSet::new(),
            };
            Change::toggle(*class, &had)
        }
    };
    // The source's element, named for the other views once.
    let target = match element {
        Element::From(source) if views.iter().any(|page| page.view != source.view) => {
            Some(name_for_others(cdp, source).await)
        }
        _ => None,
    };
    let mut outcomes = Vec::with_capacity(views.len());
    for page in views {
        let outcome = match force_one(cdp, registry, page, element, target.as_ref(), &change).await
        {
            Ok(Some(classes)) => Outcome::Forced(classes),
            Ok(None) => Outcome::Missing,
            Err(error) => Outcome::Failed(error.to_string()),
        };
        outcomes.push((page.view, outcome));
    }
    outcomes
}

async fn name_for_others(cdp: &Arc<CdpRouter>, source: &Source) -> Result<Value, String> {
    let (session, context) = source
        .agent
        .as_ref()
        .ok_or_else(|| format!("view {} has no sync agent to name the element", source.view))?;
    cdp::agent_target(cdp, session, *context, source.backend)
        .await
        .map_err(|error| error.to_string())
}

/// `None`: the view has no such element.
async fn force_one(
    cdp: &Arc<CdpRouter>,
    registry: &Mutex<Registry>,
    page: &ViewPage,
    element: &Element,
    target: Option<&Result<Value, String>>,
    change: &Change,
) -> Result<Option<BTreeSet<PseudoClass>>, EngineError> {
    let Some(page_session) = page.page.as_deref() else {
        return Err(EngineError::Invalid(format!(
            "view {} is no Chromium view: only Chromium forces states",
            page.view
        )));
    };
    let backend = match element {
        Element::From(source) if source.view == page.view => source.backend,
        Element::From(_) => {
            let target = match target {
                Some(Ok(target)) => target,
                Some(Err(error)) => return Err(EngineError::Invalid(error.clone())),
                None => return Err(EngineError::Invalid("the element has no name".to_owned())),
            };
            let (session, context) = page.agent.as_ref().ok_or_else(|| {
                EngineError::Invalid(format!("view {} has no sync agent", page.view))
            })?;
            match cdp::agent_find(cdp, session, *context, target).await? {
                Some(backend) => backend,
                None => return Ok(None),
            }
        }
        Element::Selector(selector) => {
            let (own, root) = session_of(cdp, registry, page.view, page_session).await?;
            match cdp::selector_node(cdp, &own, root, selector).await? {
                Some(backend) => backend,
                None => return Ok(None),
            }
        }
    };
    let (own, _) = session_of(cdp, registry, page.view, page_session).await?;
    let chain = cdp::chain_of(cdp, &own, backend).await?;
    let (classes, changes) = {
        let mut registry = lock(registry);
        let classes = change.applied_to(&registry.classes_of(page.view, backend));
        let changes = registry.apply(page.view, backend, chain, classes.clone());
        (classes, changes)
    };
    cdp::force(cdp, &own, &changes).await?;
    Ok(Some(classes))
}

/// rBrowser's session on `view` and its document: the one it has (the document asked for again
/// after a new one), else a new one on the target of `page_session`.
async fn session_of(
    cdp: &Arc<CdpRouter>,
    registry: &Mutex<Registry>,
    view: ViewId,
    page_session: &str,
) -> Result<(String, i64), EngineError> {
    let known = lock(registry).session(view);
    match known {
        Some((session, Some(root))) => return Ok((session, root)),
        Some((session, None)) => match cdp::document(cdp, &session).await {
            Ok(root) => {
                lock(registry).set_root(view, root);
                return Ok((session, root));
            }
            Err(error) => {
                tracing::debug!(%view, %error, "force state: the session is stale; a new one");
                lock(registry).take(view);
                cdp::detach(cdp, &session).await;
            }
        },
        None => {}
    }
    let (session, root) = cdp::open_session(cdp, page_session).await?;
    lock(registry).open(view, session.clone(), root);
    Ok((session, root))
}

/// Takes every forced state of `views` back and ends rBrowser's session on each: the views that
/// had something forced. The session goes even when a call fails, so nothing stays forced.
pub async fn reset(
    cdp: &Arc<CdpRouter>,
    registry: &Mutex<Registry>,
    views: &[ViewId],
) -> Vec<ViewId> {
    let mut reset = Vec::new();
    for view in views {
        let (changes, session) = {
            let mut registry = lock(registry);
            let changes = registry.clear(*view);
            (changes, registry.take(*view))
        };
        let Some(session) = session else {
            continue;
        };
        if !changes.is_empty() {
            reset.push(*view);
        }
        if let Err(error) = cdp::force(cdp, &session, &changes).await {
            tracing::debug!(%view, %error, "force state: the reset met a gone node; detaching");
        }
        cdp::detach(cdp, &session).await;
    }
    reset
}

#[cfg(test)]
#[path = "apply_tests.rs"]
mod tests;
