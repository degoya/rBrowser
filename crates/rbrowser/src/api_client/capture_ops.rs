//! The op `capture` (RB-096-08): requests the views captured, taken into the API client. A save
//! stores each below `.rbrowser/api/<host>/` named after its method and path - a request of the
//! same method and path once, one saved before not again; an open hands the first to the panel's
//! editor as a draft, unsaved ([`put_draft`], [`take_draft`]); otherwise the requests are only
//! answered (MCP).
//!
//! The secrets are variables ([`super::capture`]); their values go to the private values
//! (`api-client.secrets`, `config.local.json`, the `$shared` block) only when the caller may keep
//! them - the panel, with the setting `api-client.captureSecrets` at `private` - and never into
//! an answer: each variable says only whether its value was `kept`, `replaced` a different one
//! kept there before, is `present` there already or is `missing` (masked by the capture, too
//! long, or not kept). A request that holds a `{{` of the page is skipped as `template`
//! ([`Refused::Template`]). An answer masks the values
//! the sweep looks for ([`MIN_SWEPT_CHARS`] and longer): a shorter one is a variable where it
//! stood, and the same text elsewhere stays as the file holds it.

use std::collections::BTreeSet;
use std::fmt;
use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::sync::Mutex;

use rb_api_model::env::SHARED;
use rb_api_model::{EnvFile, mask::mask_json};
use rb_plugin_types::{Failure, FailureKind};
use serde_json::{Map, Value, json};

use super::capture::{
    BodyMissing, Captured, Converted, MIN_SWEPT_CHARS, Refused, convert_in, folder_of,
    untagged_folder_of,
};
use super::ops::Context;
use super::store::Store;
use super::{error, failure_code};
use crate::cdp::lock;

/// The settings key: what becomes of a captured secret's value.
pub const CAPTURE_SECRETS_KEY: &str = "api-client.captureSecrets";
/// The most requests one op takes.
pub const MAX_CAPTURES: usize = 200;
/// The longest value the private values keep (their schema's `maxLength`).
pub const MAX_SECRET_CHARS: usize = 8192;
/// The most values one block of the private values keeps (their schema's `maxProperties`).
const MAX_SECRETS: usize = 200;
/// The most captured values remembered as registered with the log's mask (the w31 security
/// review): the dedupe set is cleared when full, so it does not grow with every capture, and a
/// value past it is registered all the same (the mask itself keeps each value once).
pub const MAX_REGISTERED: usize = 1000;

/// The captured values registered with the log's mask, by a hash: never the values themselves.
static REGISTERED: Mutex<BTreeSet<u64>> = Mutex::new(BTreeSet::new());

/// Registers `value` with the log's mask (`rb_diag::redact`); the dedupe set spares the mask's
/// own search for a value seen before and holds at most [`MAX_REGISTERED`] hashes.
fn register(value: &str) {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    let key = hasher.finish();
    let mut known = lock(&REGISTERED);
    if known.contains(&key) {
        return;
    }
    if known.len() >= MAX_REGISTERED {
        known.clear();
    }
    known.insert(key);
    rb_diag::redact::register_secret(value);
}

/// What becomes of a captured secret's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretMode {
    /// Into the private values (the default).
    Private,
    /// Nowhere: the variable stays without a value.
    Drop,
}

/// The mode the settings `values` choose.
#[must_use]
pub fn mode_of(values: &Map<String, Value>) -> SecretMode {
    match values.get(CAPTURE_SECRETS_KEY).and_then(Value::as_str) {
        Some("drop") => SecretMode::Drop,
        _ => SecretMode::Private,
    }
}

/// What the caller wants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wanted {
    pub save: bool,
    /// The first request as the panel's draft (without `save`).
    pub open: bool,
    /// The captured values go to the private values.
    pub keep: bool,
}

/// An op's outcome. Its `Debug` leaves the private values out.
pub struct Taken {
    pub answer: Value,
    /// `api-client.secrets` with the captured values in, when any was kept.
    pub secrets: Option<Value>,
    /// The editor's draft, for an open.
    pub draft: Option<Value>,
}

impl fmt::Debug for Taken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Taken")
            .field("answer", &self.answer)
            .field("secrets_kept", &self.secrets.is_some())
            .field("draft", &self.draft)
            .finish()
    }
}

static DRAFT: Mutex<Option<Value>> = Mutex::new(None);

/// The draft the API client's panel takes next (its `state`).
pub fn put_draft(draft: Value) {
    *lock(&DRAFT) = Some(draft);
}

/// The draft waiting for the panel, taken.
pub fn take_draft() -> Option<Value> {
    lock(&DRAFT).take()
}

/// The private values with the captured ones in, and which names they hold now.
struct Private {
    value: Value,
    file: EnvFile,
    environment: Option<String>,
    changed: bool,
}

impl Private {
    fn of(ctx: &Context) -> Self {
        let value = match &ctx.secrets {
            Value::Object(_) => ctx.secrets.clone(),
            _ => json!({}),
        };
        Self {
            file: EnvFile::from_json(&value).unwrap_or_default(),
            value,
            environment: ctx.environment.clone(),
            changed: false,
        }
    }

    fn present(&self, name: &str) -> bool {
        self.file.get(SHARED, name).is_some()
            || self
                .environment
                .as_deref()
                .is_some_and(|environment| self.file.get(environment, name).is_some())
    }

    /// Keeps `value` under `name` in `$shared`: whether it was kept, and whether it replaced a
    /// different value there - the answer says so (`replaced`).
    fn keep(&mut self, name: &str, value: &str) -> Kept {
        if value.chars().count() > MAX_SECRET_CHARS {
            return Kept::No;
        }
        let Some(root) = self.value.as_object_mut() else {
            return Kept::No;
        };
        let shared = root.entry(SHARED.to_owned()).or_insert_with(|| json!({}));
        let Some(shared) = shared.as_object_mut() else {
            return Kept::No;
        };
        if !shared.contains_key(name) && shared.len() >= MAX_SECRETS {
            return Kept::No;
        }
        let before = shared.get(name);
        if before.and_then(Value::as_str) == Some(value) {
            return Kept::Yes;
        }
        let replaced = before.is_some();
        shared.insert(name.to_owned(), Value::String(value.to_owned()));
        self.changed = true;
        if replaced { Kept::Replaced } else { Kept::Yes }
    }
}

/// What [`Private::keep`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kept {
    No,
    Yes,
    /// Kept in place of a different value.
    Replaced,
}

/// The variables of `converted`, each named once, with what became of its value.
fn variables(
    converted: &[&Converted],
    private: &mut Private,
    keep: bool,
    secrets: &mut Vec<String>,
) -> Vec<Value> {
    let mut out: Vec<Value> = Vec::new();
    let mut named = BTreeSet::new();
    for variable in converted.iter().flat_map(|one| one.variables.iter()) {
        if let Some(value) = &variable.value {
            register(value);
            if value.chars().count() >= MIN_SWEPT_CHARS {
                secrets.push(value.clone());
            }
        }
        if !named.insert(variable.name.clone()) {
            continue;
        }
        let kept = match variable.value.as_deref() {
            Some(value) if keep => private.keep(&variable.name, value),
            _ => Kept::No,
        };
        let state = match kept {
            Kept::Replaced => "replaced",
            Kept::Yes => "kept",
            Kept::No if private.present(&variable.name) => "present",
            Kept::No => "missing",
        };
        out.push(json!({ "name": variable.name, "value": state }));
    }
    out
}

/// The folder of saved request `id`.
fn folder_of_id(id: &str) -> &str {
    id.rsplit_once('/').map_or("", |(folder, _)| folder)
}

/// The folder of an authority's captured requests: [`folder_of`]'s, or the untagged name a
/// capture made before the tag when that folder exists, the tagged one does not, and every
/// request in it goes to this authority - a host never shares another's folder.
fn folder_chooser(store: &Store) -> impl Fn(&str) -> String {
    let folders: BTreeSet<String> = store.folders().unwrap_or_default().into_iter().collect();
    let hosts: Vec<(String, String)> = store
        .list()
        .unwrap_or_default()
        .into_iter()
        .map(|listed| {
            let host = rb_util::url::authority_of(&listed.url).unwrap_or_default();
            (
                folder_of_id(&listed.id).to_owned(),
                host.to_ascii_lowercase(),
            )
        })
        .collect();
    move |authority: &str| {
        let tagged = folder_of(authority);
        let lower = authority.to_ascii_lowercase();
        match untagged_folder_of(authority) {
            Some(old)
                if folders.contains(&old)
                    && !folders.contains(&tagged)
                    && hosts
                        .iter()
                        .filter(|(folder, _)| *folder == old)
                        .all(|(_, host)| *host == lower) =>
            {
                old
            }
            _ => tagged,
        }
    }
}

/// The path of a saved request's URL template (`{{baseUrl}}/users?x` is `/users`).
fn path_of_template(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    rest.find('/')
        .map_or_else(|| "/".to_owned(), |at| rest[at..].to_owned())
}

fn one(converted: &Converted, variables: Vec<Value>) -> Value {
    json!({
        "request": converted.request,
        "folder": converted.folder,
        "variables": variables,
        "bodyMissing": converted.body_missing.map(BodyMissing::id),
        "incomplete": converted.incomplete,
    })
}

/// Takes `captured` (in the order asked, `None` for a request the views no longer hold) as
/// `wanted`.
///
/// # Errors
///
/// `no_project`, `untrusted`, `nothing_captured`, `too_many_captures` with the most,
/// `capture_gone` for an open with nothing to open (`capture_template` when what there was holds
/// a `{{` of the page), and [`Store::list`]'s for a save.
pub fn run(
    ctx: &Context,
    captured: Vec<Option<Captured>>,
    wanted: Wanted,
) -> Result<Taken, Failure> {
    let root = ctx
        .root
        .clone()
        .ok_or_else(|| error(FailureKind::NotFound, "no_project"))?;
    if !ctx.trusted {
        return Err(error(FailureKind::AccessDenied, "untrusted"));
    }
    if captured.is_empty() {
        return Err(error(FailureKind::BadData, "nothing_captured"));
    }
    if captured.len() > MAX_CAPTURES {
        return Err(error(FailureKind::Limit, "too_many_captures")
            .with_param("most", MAX_CAPTURES.to_string()));
    }
    // A request the views no longer hold is `gone`; one not taken says why (`Refused::id`).
    let store = Store::new(&root);
    let folder = folder_chooser(&store);
    let converted: Vec<Result<Converted, &'static str>> = captured
        .iter()
        .map(|one| match one {
            Some(one) => convert_in(one, &folder).map_err(Refused::id),
            None => Err("gone"),
        })
        .collect();
    let mut private = Private::of(ctx);
    let mut secrets = Vec::new();
    let (answer, draft) = if wanted.save {
        let store = Store::new(&root);
        (
            save(&store, &converted, &mut private, wanted.keep, &mut secrets)?,
            None,
        )
    } else {
        let first = converted
            .iter()
            .find_map(|one| one.as_ref().ok())
            .ok_or_else(|| {
                let template = Refused::Template.id();
                if converted
                    .iter()
                    .any(|one| matches!(one, Err(reason) if *reason == template))
                {
                    error(FailureKind::BadData, "capture_template")
                } else {
                    error(FailureKind::NotFound, "capture_gone")
                }
            })?;
        let list = variables(&[first], &mut private, wanted.keep, &mut secrets);
        let answer = one(first, list);
        let draft = wanted.open.then(|| answer.clone());
        (answer, draft)
    };
    let answer = mask_json(&answer, &secrets);
    Ok(Taken {
        draft: draft.map(|draft| mask_json(&draft, &secrets)),
        answer,
        secrets: private.changed.then_some(private.value),
    })
}

/// The private values could not be written after the op (the files hold the variables only, so
/// no value is left behind): no variable says `kept` any more - it is `missing` - and the answer
/// and the draft carry `warning: secrets_not_saved`.
pub fn not_kept(taken: &mut Taken) {
    taken.secrets = None;
    for value in std::iter::once(&mut taken.answer).chain(taken.draft.as_mut()) {
        let Value::Object(fields) = value else {
            continue;
        };
        if let Some(Value::Array(variables)) = fields.get_mut("variables") {
            for variable in variables {
                if let Some(state) = variable.get_mut("value")
                    && (*state == "kept" || *state == "replaced")
                {
                    *state = json!("missing");
                }
            }
        }
        fields.insert("warning".to_owned(), json!("secrets_not_saved"));
    }
}

fn save(
    store: &Store,
    converted: &[Result<Converted, &'static str>],
    private: &mut Private,
    keep: bool,
    secrets: &mut Vec<String>,
) -> Result<Value, Failure> {
    let mut taken: BTreeSet<(String, String, String)> = store
        .list()?
        .into_iter()
        .map(|listed| {
            (
                folder_of_id(&listed.id).to_owned(),
                listed.method.to_ascii_uppercase(),
                path_of_template(&listed.url),
            )
        })
        .collect();
    let before = taken.clone();
    let (mut saved, mut skipped, mut written) = (Vec::new(), Vec::new(), Vec::new());
    for one in converted {
        let one = match one {
            Ok(one) => one,
            Err(reason) => {
                skipped.push(json!({ "name": Value::Null, "reason": reason }));
                continue;
            }
        };
        let key = (
            one.folder.clone(),
            one.request.method.clone(),
            one.path.clone(),
        );
        if taken.contains(&key) {
            let reason = if before.contains(&key) {
                "exists"
            } else {
                "duplicate"
            };
            skipped.push(json!({ "name": one.request.name, "reason": reason }));
            continue;
        }
        match store.save(None, Some(&one.folder), &one.request) {
            Ok(id) => {
                taken.insert(key);
                saved.push(json!({ "id": id, "name": one.request.name,
                                   "bodyMissing": one.body_missing.map(BodyMissing::id),
                                   "incomplete": one.incomplete }));
                written.push(one);
            }
            Err(failure) => {
                skipped.push(json!({ "name": one.request.name, "reason": failure_code(&failure) }));
            }
        }
    }
    let variables = variables(&written, private, keep, secrets);
    Ok(json!({ "saved": saved, "skipped": skipped, "variables": variables }))
}

#[cfg(test)]
#[path = "capture_ops_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "capture_folder_tests.rs"]
mod folder_tests;
