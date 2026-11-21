//! Editing a recording's steps (RB-092-01): a step added at a place, changed by a patch, switched
//! off and on, removed; the recording checked before it is written; and the parts of it that
//! play - without the steps switched off, from a step on, up to a step, one step alone. The
//! Recorder panel edits its draft in the shell (`ui/src/recorder/recordingEdit.ts`) and the core
//! checks it here on save; the MCP tools `recorder_step_*` edit the stored file through these.

use serde::{Deserialize, Deserializer};

use super::model::{Action, MAX_PAUSE_MS, Recording, Step};
use super::record::MAX_STEPS;

/// Why an edit was refused; safe to show.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EditError {
    /// No step has the id.
    #[error("the recording has no step {0}")]
    NoStep(u32),
    /// Two steps have the id.
    #[error("two steps have the id {0}")]
    DuplicateId(u32),
    /// A pause before a step above [`MAX_PAUSE_MS`].
    #[error("step {step}: the pause before it is {ms} ms, at most {MAX_PAUSE_MS} ms")]
    Pause {
        /// The step.
        step: u32,
        /// The pause.
        ms: u32,
    },
    /// A fixed wait above [`MAX_PAUSE_MS`].
    #[error("step {step} waits {ms} ms, at most {MAX_PAUSE_MS} ms")]
    Wait {
        /// The step.
        step: u32,
        /// The wait.
        ms: u32,
    },
    /// More than [`MAX_STEPS`] steps.
    #[error("a recording holds at most {MAX_STEPS} steps")]
    TooMany,
    /// An edit that adds or changes a secret field's fill ([`check_secret`]).
    #[error(
        "step {0}: a secret field's fill is the recorder's own - only a recording or the Recorder \
         panel adds or changes one"
    )]
    Secret(u32),
}

/// Where a step goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// Before the first step.
    Start,
    /// Right after the step with this id.
    After(u32),
    /// After the last step.
    End,
}

impl Place {
    /// The place an `after` argument names: absent the end, `0` the start, else after that step.
    #[must_use]
    pub fn from_after(after: Option<u32>) -> Self {
        match after {
            None => Place::End,
            Some(0) => Place::Start,
            Some(id) => Place::After(id),
        }
    }
}

/// A change of one step; an absent field stays, `null` takes a name or a pause away.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepPatch {
    /// The name; `null` or empty takes it away.
    #[serde(default, deserialize_with = "present")]
    pub name: Option<Option<String>>,
    /// What the step does.
    #[serde(default)]
    pub action: Option<Action>,
    /// The breakpoints it runs in; empty: all.
    #[serde(default)]
    pub only_in: Option<Vec<String>>,
    /// Switched off.
    #[serde(default)]
    pub disabled: Option<bool>,
    /// The pause before it, ms; `null` the measured one.
    #[serde(default, deserialize_with = "present")]
    pub pause_ms: Option<Option<u32>>,
}

/// A field that is there, `null` included: `Some(None)` for `null`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// The id a new step gets: one above the highest.
#[must_use]
pub fn next_id(recording: &Recording) -> u32 {
    recording
        .steps
        .iter()
        .map(|step| step.id)
        .max()
        .map_or(1, |id| id.saturating_add(1))
}

fn index_of(recording: &Recording, id: u32) -> Result<usize, EditError> {
    recording
        .steps
        .iter()
        .position(|step| step.id == id)
        .ok_or(EditError::NoStep(id))
}

/// A step doing `action` for `place`: the next id, the view and time of the step before it (of
/// the first step at the start, of the recording's first breakpoint when it has none), the
/// pause `pause_ms`.
///
/// # Errors
///
/// [`EditError::NoStep`] for a place after a step that is not there.
pub fn new_step(
    recording: &Recording,
    place: Place,
    action: Action,
    pause_ms: Option<u32>,
) -> Result<Step, EditError> {
    let before = match place {
        Place::Start => recording.steps.first(),
        Place::After(id) => Some(&recording.steps[index_of(recording, id)?]),
        Place::End => recording.steps.last(),
    };
    let view = before.map_or_else(
        || {
            recording
                .breakpoints
                .first()
                .map(|breakpoint| breakpoint.id.clone())
                .unwrap_or_default()
        },
        |step| step.view.clone(),
    );
    Ok(Step {
        id: next_id(recording),
        name: None,
        action,
        view,
        at_ms: before.map_or(0, |step| step.at_ms),
        only_in: Vec::new(),
        path: Vec::new(),
        disabled: false,
        pause_ms,
    })
}

/// Puts `step` at `place`; its index.
///
/// # Errors
///
/// No such step, an id already taken, too many steps.
pub fn insert(recording: &mut Recording, place: Place, step: Step) -> Result<usize, EditError> {
    if recording.steps.iter().any(|known| known.id == step.id) {
        return Err(EditError::DuplicateId(step.id));
    }
    if recording.steps.len() >= MAX_STEPS {
        return Err(EditError::TooMany);
    }
    let at = match place {
        Place::Start => 0,
        Place::After(id) => index_of(recording, id)? + 1,
        Place::End => recording.steps.len(),
    };
    recording.steps.insert(at, step);
    Ok(at)
}

/// Changes step `id` as `patch` says.
///
/// # Errors
///
/// [`EditError::NoStep`].
pub fn apply(recording: &mut Recording, id: u32, patch: &StepPatch) -> Result<(), EditError> {
    let at = index_of(recording, id)?;
    let step = &mut recording.steps[at];
    if let Some(name) = &patch.name {
        step.name = name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned);
    }
    if let Some(action) = &patch.action {
        step.action = action.clone();
    }
    if let Some(only_in) = &patch.only_in {
        step.only_in.clone_from(only_in);
    }
    if let Some(disabled) = patch.disabled {
        step.disabled = disabled;
    }
    if let Some(pause_ms) = patch.pause_ms {
        step.pause_ms = pause_ms;
    }
    Ok(())
}

/// Switches step `id` off (`true`) or on.
///
/// # Errors
///
/// [`EditError::NoStep`].
pub fn set_disabled(recording: &mut Recording, id: u32, disabled: bool) -> Result<(), EditError> {
    let at = index_of(recording, id)?;
    recording.steps[at].disabled = disabled;
    Ok(())
}

/// Takes step `id` out; the step.
///
/// # Errors
///
/// [`EditError::NoStep`].
pub fn remove(recording: &mut Recording, id: u32) -> Result<Step, EditError> {
    let at = index_of(recording, id)?;
    Ok(recording.steps.remove(at))
}

/// Whether `action` is a secret field's fill (its value a `{{secret:<name>}}` placeholder).
fn is_secret(action: &Action) -> bool {
    matches!(action, Action::Fill { secret: true, .. })
}

/// An agent's edit of step `id` from `before` (none for a new step) to `after` (RB-092-01): a
/// secret fill is the recorder's own - an edit neither adds one nor changes one (its field, its
/// secret, the flag), so a trusted replay never types a secret into an element an agent chose.
/// A secret fill left exactly as it was passes.
///
/// # Errors
///
/// [`EditError::Secret`].
pub fn check_secret(id: u32, before: Option<&Action>, after: &Action) -> Result<(), EditError> {
    let touches = is_secret(after) || before.is_some_and(is_secret);
    if touches && before != Some(after) {
        return Err(EditError::Secret(id));
    }
    Ok(())
}

/// What the model's types cannot hold: unique ids, at most [`MAX_STEPS`] steps, pauses and waits
/// at most [`MAX_PAUSE_MS`].
///
/// # Errors
///
/// The first thing wrong.
pub fn check(recording: &Recording) -> Result<(), EditError> {
    if recording.steps.len() > MAX_STEPS {
        return Err(EditError::TooMany);
    }
    let mut ids: Vec<u32> = recording.steps.iter().map(|step| step.id).collect();
    ids.sort_unstable();
    if let Some(pair) = ids.windows(2).find(|pair| pair[0] == pair[1]) {
        return Err(EditError::DuplicateId(pair[0]));
    }
    for step in &recording.steps {
        if let Some(ms) = step.pause_ms.filter(|ms| *ms > MAX_PAUSE_MS) {
            return Err(EditError::Pause { step: step.id, ms });
        }
        if let Action::Wait { ms } = step.action
            && ms > MAX_PAUSE_MS
        {
            return Err(EditError::Wait { step: step.id, ms });
        }
    }
    Ok(())
}

fn with_steps(recording: &Recording, steps: Vec<Step>) -> Recording {
    Recording {
        steps,
        ..recording.clone()
    }
}

/// The recording as it plays and exports: without the steps switched off.
#[must_use]
pub fn playable(recording: &Recording) -> Recording {
    let steps = recording
        .steps
        .iter()
        .filter(|step| !step.disabled)
        .cloned()
        .collect();
    with_steps(recording, steps)
}

/// The playable steps from step `id` on ("Play from here").
///
/// # Errors
///
/// [`EditError::NoStep`].
pub fn from_step(recording: &Recording, id: u32) -> Result<Recording, EditError> {
    let at = index_of(recording, id)?;
    let steps = recording.steps[at..]
        .iter()
        .filter(|step| !step.disabled)
        .cloned()
        .collect();
    Ok(with_steps(recording, steps))
}

/// The playable steps up to step `id` and it (played before "Continue recording from here").
///
/// # Errors
///
/// [`EditError::NoStep`].
pub fn up_to(recording: &Recording, id: u32) -> Result<Recording, EditError> {
    let at = index_of(recording, id)?;
    let steps = recording.steps[..=at]
        .iter()
        .filter(|step| !step.disabled)
        .cloned()
        .collect();
    Ok(with_steps(recording, steps))
}

/// Step `id` alone, switched on and without a pause ("Try"): the person asked for it.
///
/// # Errors
///
/// [`EditError::NoStep`].
pub fn only_step(recording: &Recording, id: u32) -> Result<Recording, EditError> {
    let at = index_of(recording, id)?;
    let mut step = recording.steps[at].clone();
    step.disabled = false;
    step.pause_ms = None;
    Ok(with_steps(recording, vec![step]))
}

/// `base` continued from step `after` ("Continue recording from here"): the steps of `segment` -
/// a recording that started there - after it, with ids from [`next_id`] on and times from that
/// step's on; the steps after it move by the segment's length, so the pauses between them stay.
/// The segment's breakpoints that `base` lacks join it.
///
/// # Errors
///
/// [`EditError::NoStep`]; [`EditError::TooMany`].
pub fn splice(base: &Recording, after: u32, segment: &Recording) -> Result<Recording, EditError> {
    let at = index_of(base, after)?;
    if base.steps.len() + segment.steps.len() > MAX_STEPS {
        return Err(EditError::TooMany);
    }
    let anchor_ms = base.steps[at].at_ms;
    let length_ms = segment
        .steps
        .iter()
        .map(|step| step.at_ms)
        .max()
        .unwrap_or(0);
    let mut id = next_id(base);
    let mut spliced = base.clone();
    let new_steps = segment.steps.iter().map(|step| {
        let mut step = step.clone();
        step.id = id;
        id = id.saturating_add(1);
        step.at_ms = anchor_ms.saturating_add(step.at_ms);
        step
    });
    let tail: Vec<Step> = spliced.steps.split_off(at + 1);
    spliced.steps.extend(new_steps);
    spliced.steps.extend(tail.into_iter().map(|mut step| {
        step.at_ms = step.at_ms.saturating_add(length_ms);
        step
    }));
    for breakpoint in &segment.breakpoints {
        if !spliced
            .breakpoints
            .iter()
            .any(|known| known.id == breakpoint.id)
        {
            spliced.breakpoints.push(breakpoint.clone());
        }
    }
    Ok(spliced)
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
