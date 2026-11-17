//! Which views a copy reads and writes (RB-040-05), from the views of the moment: the Firefox
//! and WebKit views of the target, grouped by session, each group read through a view of its
//! own session. With a `source`, only the views of that session get cookies; one of another
//! session is reported, never filled with a foreign session's cookies.

use std::collections::BTreeMap;

use rb_config::EngineKind;
use rb_engine::{ViewId, ViewInfo};

use super::filter::cookie_host;
use super::model::{CopyArgs, Source, Targets, ViewReport};

/// One session's part of a copy.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Group {
    /// The session.
    pub session: String,
    /// The views its cookies are read through (CDP), Chromium's own views first.
    pub readers: Vec<ViewId>,
    /// Its Firefox views that get the cookies (one jar for them all).
    pub firefox: Vec<ViewId>,
    /// Its WebKit views that get the cookies (one jar each).
    pub webkit: Vec<ViewId>,
    /// The hosts its views show: the sites of the domain filter.
    pub hosts: Vec<String>,
}

/// What a copy does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plan {
    /// Per session.
    pub groups: Vec<Group>,
    /// Target views left out: they belong to another session than the one asked for.
    pub other_session: Vec<ViewReport>,
}

/// Why there is nothing to plan.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// No Firefox or WebKit view is open (or in the target): a note, not a failure.
    #[error("there is no Firefox or WebKit view to copy cookies into")]
    NoExternalViews,
    /// The target names a view that is not open.
    #[error("unknown view {0}")]
    UnknownView(ViewId),
    /// The target names a Chromium view, which has the session's cookies already.
    #[error("view {0} is a Chromium view; cookies are copied into Firefox and WebKit views")]
    NotExternal(ViewId),
    /// The source session has no view to read its cookies through.
    #[error("no view of session \"{0}\" is open to read its cookies")]
    NoReader(String),
}

/// The plan of `args` over `views`.
pub fn plan(views: &[ViewInfo], args: &CopyArgs) -> Result<Plan, PlanError> {
    let named: Vec<ViewId> = match &args.target {
        Some(Targets::One(rb_engine::Target::View(view))) => vec![*view],
        Some(Targets::Many(list)) => list.clone(),
        Some(Targets::One(rb_engine::Target::All)) | None => Vec::new(),
    };
    for id in &named {
        match views.iter().find(|view| view.id == *id) {
            None => return Err(PlanError::UnknownView(*id)),
            Some(view) if view.engine == EngineKind::Chromium => {
                return Err(PlanError::NotExternal(*id));
            }
            Some(_) => {}
        }
    }
    let covered = |view: &ViewInfo| match &args.target {
        Some(target) => target.covers(view.id),
        None => true,
    };
    let targets: Vec<&ViewInfo> = views
        .iter()
        .filter(|view| view.engine != EngineKind::Chromium && covered(view))
        .collect();
    if targets.is_empty() {
        return Err(PlanError::NoExternalViews);
    }
    let source = match &args.source {
        None => None,
        Some(Source::Session(session)) => Some((session.clone(), None)),
        Some(Source::View(id)) => {
            let view = views
                .iter()
                .find(|view| view.id == *id)
                .ok_or(PlanError::UnknownView(*id))?;
            Some((view.session.clone(), Some(*id)))
        }
    };
    if let Some((session, _)) = &source
        && !views.iter().any(|view| view.session == *session)
    {
        return Err(PlanError::NoReader(session.clone()));
    }
    let mut sessions: BTreeMap<String, Group> = BTreeMap::new();
    let mut other_session = Vec::new();
    for view in targets {
        if let Some((wanted, _)) = &source
            && view.session != *wanted
        {
            other_session.push(ViewReport {
                view: view.id,
                engine: view.engine,
                session: view.session.clone(),
                copied: 0,
                refused: 0,
                skipped: BTreeMap::new(),
                reloaded: false,
                error: Some(rb_i18n::t_with(
                    "cookies.error.otherSession",
                    &[("session", view.session.as_str()), ("source", wanted)],
                )),
            });
            continue;
        }
        let group = sessions
            .entry(view.session.clone())
            .or_insert_with(|| Group {
                session: view.session.clone(),
                ..Group::default()
            });
        match view.engine {
            EngineKind::Firefox => group.firefox.push(view.id),
            EngineKind::Webkit => group.webkit.push(view.id),
            EngineKind::Chromium => {}
        }
    }
    let groups = sessions
        .into_values()
        .map(|mut group| {
            let first = source.as_ref().and_then(|(_, first)| *first);
            group.readers = readers(views, &group.session, first);
            group.hosts = hosts(views, &group.session);
            group
        })
        .collect();
    Ok(Plan {
        groups,
        other_session,
    })
}

/// The views of `session` to read its cookies through: `first` (the source view), then the
/// Chromium views, then the grid stand-ins of its Firefox and WebKit views (the same CEF
/// request context).
fn readers(views: &[ViewInfo], session: &str, first: Option<ViewId>) -> Vec<ViewId> {
    let mut readers: Vec<ViewId> = first.into_iter().collect();
    let mut ordered: Vec<&ViewInfo> = views
        .iter()
        .filter(|view| view.session == session)
        .collect();
    ordered.sort_by_key(|view| view.engine != EngineKind::Chromium);
    for view in ordered {
        if !readers.contains(&view.id) {
            readers.push(view.id);
        }
    }
    readers
}

/// The hosts the views of `session` show, sorted, once each.
fn hosts(views: &[ViewInfo], session: &str) -> Vec<String> {
    let mut hosts: Vec<String> = views
        .iter()
        .filter(|view| view.session == session)
        .filter_map(|view| cookie_host(&view.url))
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod tests;
