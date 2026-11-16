//! What rBrowser forces, per view (RB-090-53): the elements ("anchors") with their states and the
//! nodes each one reaches - the element itself and, for `:hover`, `:active` and `:focus-within`,
//! every ancestor up to `<html>`, as a real hover does - so `CSS.forcePseudoState` gets each
//! node's whole set, and a reset or a removal knows what to take back. Pure: the CDP calls are
//! `cdp.rs`, the node ids are those of rBrowser's own session on the view (`session`).

use std::collections::{BTreeMap, BTreeSet};

use rb_engine::ViewId;

/// A state `CSS.forcePseudoState` forces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PseudoClass {
    Hover,
    Active,
    Focus,
    FocusVisible,
    FocusWithin,
    Visited,
    Target,
}

impl PseudoClass {
    /// Every state, in menu order.
    pub const ALL: [Self; 7] = [
        Self::Hover,
        Self::Active,
        Self::Focus,
        Self::FocusVisible,
        Self::FocusWithin,
        Self::Visited,
        Self::Target,
    ];

    /// CDP's word for it (`forcedPseudoClasses`), the pseudo-class without its colon.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Hover => "hover",
            Self::Active => "active",
            Self::Focus => "focus",
            Self::FocusVisible => "focus-visible",
            Self::FocusWithin => "focus-within",
            Self::Visited => "visited",
            Self::Target => "target",
        }
    }

    /// `hover` or `:hover`, any case; `None` for a state rBrowser does not force.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let word = text.trim();
        let word = word.strip_prefix(':').unwrap_or(word);
        Self::ALL
            .into_iter()
            .find(|class| class.name().eq_ignore_ascii_case(word))
    }

    /// Whether a real browser gives it to every ancestor of the element too.
    #[must_use]
    pub fn reaches_ancestors(self) -> bool {
        matches!(self, Self::Hover | Self::Active | Self::FocusWithin)
    }
}

/// What a request does to an element's states.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// Exactly these (none: the element's states go).
    Set(BTreeSet<PseudoClass>),
    /// One more.
    Add(PseudoClass),
    /// One less.
    Remove(PseudoClass),
}

impl Change {
    /// The menu's switch: off when the element in the menu's view has `class`, else on.
    #[must_use]
    pub fn toggle(class: PseudoClass, had: &BTreeSet<PseudoClass>) -> Self {
        if had.contains(&class) {
            Self::Remove(class)
        } else {
            Self::Add(class)
        }
    }

    /// The states an element that had `had` has after the change.
    #[must_use]
    pub fn applied_to(&self, had: &BTreeSet<PseudoClass>) -> BTreeSet<PseudoClass> {
        match self {
            Self::Set(classes) => classes.clone(),
            Self::Add(class) => had.iter().copied().chain([*class]).collect(),
            Self::Remove(class) => had.iter().copied().filter(|have| have != class).collect(),
        }
    }
}

/// One node's whole new set, for `CSS.forcePseudoState` (empty: nothing forced any more).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeChange {
    pub node: i64,
    pub classes: Vec<&'static str>,
}

/// The menu's check marks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ForcedMarks {
    /// The states forced on an element of the clicked view.
    pub this_view: BTreeSet<PseudoClass>,
    /// The states every view has forced.
    pub all_views: BTreeSet<PseudoClass>,
    /// Some view has a forced state.
    pub any_view: bool,
}

/// One forced element.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Anchor {
    /// Its backend node id (the same in every session).
    backend: i64,
    /// The node ids in rBrowser's session: the element first, then its ancestors.
    chain: Vec<i64>,
    classes: BTreeSet<PseudoClass>,
}

/// A view's forced elements and rBrowser's session on it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ViewForced {
    session: String,
    /// The document's node id in the session; `None` after a new document until asked again.
    root: Option<i64>,
    anchors: Vec<Anchor>,
}

impl ViewForced {
    /// Each node's set: an element's states on it, the ones that reach ancestors on those.
    fn nodes(&self) -> BTreeMap<i64, BTreeSet<PseudoClass>> {
        let mut nodes: BTreeMap<i64, BTreeSet<PseudoClass>> = BTreeMap::new();
        for anchor in &self.anchors {
            for (index, node) in anchor.chain.iter().enumerate() {
                let reached = anchor
                    .classes
                    .iter()
                    .copied()
                    .filter(|class| index == 0 || class.reaches_ancestors());
                nodes.entry(*node).or_default().extend(reached);
            }
        }
        nodes.retain(|_, classes| !classes.is_empty());
        nodes
    }
}

/// Every view's forced elements.
#[derive(Debug, Default)]
pub struct Registry {
    views: BTreeMap<ViewId, ViewForced>,
}

impl Registry {
    /// rBrowser's session on `view` and its document's node id, if it has one.
    #[must_use]
    pub fn session(&self, view: ViewId) -> Option<(String, Option<i64>)> {
        self.views
            .get(&view)
            .map(|entry| (entry.session.clone(), entry.root))
    }

    /// `view` has `session` now, with the document `root`, nothing forced yet.
    pub fn open(&mut self, view: ViewId, session: String, root: i64) {
        self.views.insert(
            view,
            ViewForced {
                session,
                root: Some(root),
                anchors: Vec::new(),
            },
        );
    }

    /// `view`'s document was asked for again after a new one.
    pub fn set_root(&mut self, view: ViewId, root: i64) {
        if let Some(entry) = self.views.get_mut(&view) {
            entry.root = Some(root);
        }
    }

    /// Forgets `view` and answers its session, for the detach.
    pub fn take(&mut self, view: ViewId) -> Option<String> {
        self.views.remove(&view).map(|entry| entry.session)
    }

    /// Gives the element `backend` of `view` (`chain`: its node and its ancestors' in the
    /// session) the states `classes` - none drops it - and answers every node whose set changed.
    pub fn apply(
        &mut self,
        view: ViewId,
        backend: i64,
        chain: Vec<i64>,
        classes: BTreeSet<PseudoClass>,
    ) -> Vec<NodeChange> {
        let Some(entry) = self.views.get_mut(&view) else {
            return Vec::new();
        };
        let before = entry.nodes();
        entry.anchors.retain(|anchor| anchor.backend != backend);
        if !classes.is_empty() {
            entry.anchors.push(Anchor {
                backend,
                chain,
                classes,
            });
        }
        changes(&before, &entry.nodes())
    }

    /// Takes every state of `view` back: the nodes to empty.
    pub fn clear(&mut self, view: ViewId) -> Vec<NodeChange> {
        let Some(entry) = self.views.get_mut(&view) else {
            return Vec::new();
        };
        let before = entry.nodes();
        entry.anchors.clear();
        changes(&before, &BTreeMap::new())
    }

    /// The states of the element `backend` of `view`.
    #[must_use]
    pub fn classes_of(&self, view: ViewId, backend: i64) -> BTreeSet<PseudoClass> {
        self.views
            .get(&view)
            .and_then(|entry| {
                entry
                    .anchors
                    .iter()
                    .find(|anchor| anchor.backend == backend)
            })
            .map(|anchor| anchor.classes.clone())
            .unwrap_or_default()
    }

    /// The states forced on any element of `view`.
    #[must_use]
    pub fn forced_in(&self, view: ViewId) -> BTreeSet<PseudoClass> {
        self.views
            .get(&view)
            .map(|entry| {
                entry
                    .anchors
                    .iter()
                    .flat_map(|anchor| anchor.classes.iter().copied())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The views with a forced element, in order.
    #[must_use]
    pub fn forced_views(&self) -> Vec<ViewId> {
        self.views
            .iter()
            .filter(|(_, entry)| !entry.anchors.is_empty())
            .map(|(view, _)| *view)
            .collect()
    }

    /// The menu's marks for `view` among `views` (every view of the window).
    #[must_use]
    pub fn marks(&self, view: ViewId, views: &[ViewId]) -> ForcedMarks {
        let mut all: Option<BTreeSet<PseudoClass>> = None;
        for other in views {
            let forced = self.forced_in(*other);
            all = Some(match all {
                None => forced,
                Some(common) => common.intersection(&forced).copied().collect(),
            });
        }
        ForcedMarks {
            this_view: self.forced_in(view),
            all_views: all.unwrap_or_default(),
            any_view: !self.forced_views().is_empty(),
        }
    }

    /// `DOM.documentUpdated` on `session`: Chromium dropped the nodes and their forced states,
    /// so the view's anchors and document go. The view, and whether something was forced;
    /// `None` for a session not rBrowser's (DevTools' own).
    pub fn document_updated(&mut self, session: &str) -> Option<(ViewId, bool)> {
        let (view, entry) = self
            .views
            .iter_mut()
            .find(|(_, entry)| entry.session == session)?;
        let had = !entry.anchors.is_empty();
        entry.anchors.clear();
        entry.root = None;
        Some((*view, had))
    }

    /// `session` was detached (the view closed, the renderer went): the view is forgotten.
    pub fn detached(&mut self, session: &str) -> Option<(ViewId, bool)> {
        let view = self
            .views
            .iter()
            .find(|(_, entry)| entry.session == session)
            .map(|(view, _)| *view)?;
        let entry = self.views.remove(&view)?;
        Some((view, !entry.anchors.is_empty()))
    }
}

/// Every node whose set differs between `before` and `after`, with its new set.
fn changes(
    before: &BTreeMap<i64, BTreeSet<PseudoClass>>,
    after: &BTreeMap<i64, BTreeSet<PseudoClass>>,
) -> Vec<NodeChange> {
    let nodes: BTreeSet<i64> = before.keys().chain(after.keys()).copied().collect();
    nodes
        .into_iter()
        .filter(|node| before.get(node) != after.get(node))
        .map(|node| NodeChange {
            node,
            classes: after
                .get(&node)
                .map(|classes| classes.iter().map(|class| class.name()).collect())
                .unwrap_or_default(),
        })
        .collect()
}

#[cfg(test)]
#[path = "model_tests.rs"]
mod tests;
