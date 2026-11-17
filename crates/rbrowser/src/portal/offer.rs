//! The portal's offer for an unmapped loopback origin (RB-090-71): a page served through the
//! portal names `http://localhost:5173` (a Vite dev server, say), which the shown environment
//! does not list in `assetOrigins` - the device cannot reach it, its images and scripts are
//! missing. The portal hands each such origin, exactly as the page names it, to a sink
//! ([`set_sink`], the app's status message with a button); [`OfferBook`] keeps what was offered
//! (again after a start of the portal and after another environment is shown) and what waits for
//! a click. The button runs `portal.add_asset_origin`, which adds the origin to that one
//! environment's list in the file the list comes from ([`target_file`], [`with_file_origin`];
//! `super::add_origin`). Only a person's click writes the config: no MCP tool runs the command
//! ([`refuse_mcp`]). Pure but for the sink.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use rb_commands::CommandError;
use rb_config::{ConfigFile, Environment};
use rb_sync::nav_url::origin_of;
use serde_json::{Value, json};

use super::origin_map::OriginMap;
use super::origins::Upstream;
use crate::cdp::lock;

/// `Leader S O`: adds an unmapped loopback origin to the shown environment's `assetOrigins`
/// (`{ "origin", "environment" }`; without them the oldest origin offered and not added yet).
pub const ADD_ASSET_ORIGIN: &str = "portal.add_asset_origin";
/// Why `portal.add_asset_origin` has no MCP tool.
pub const ADD_OMITTED: &str = "a person's decision: it writes the project config";
/// The settings key the command writes, as the settings window does.
pub const ENVIRONMENTS_KEY: &str = "environments";
/// The field of an environment that holds its asset origins.
const ASSET_ORIGINS: &str = "assetOrigins";
/// The most origins one shown environment is offered.
const MAX_OFFERED: usize = 64;
/// The most offers that wait for a click.
const MAX_PENDING: usize = 16;

/// Whether `origin` is an `http` origin of `localhost`, `127.0.0.1` or `[::1]`.
pub(super) fn is_loopback(origin: &str) -> bool {
    Upstream::parse(origin)
        .is_some_and(|upstream| matches!(upstream.host(), "localhost" | "127.0.0.1" | "[::1]"))
}

/// The environment shown while the page's origin is `page_origin`: the first whose URL has it.
#[must_use]
pub fn environment_for<'a>(
    environments: &'a [Environment],
    page_origin: &str,
) -> Option<&'a Environment> {
    environments
        .iter()
        .find(|env| origin_of(&env.url).as_deref() == Some(page_origin))
}

/// Why an origin is not added.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AddError {
    /// Not a bare `http` origin of `localhost`, `127.0.0.1` or `[::1]`.
    #[error("the origin is no bare http origin of localhost, 127.0.0.1 or [::1]")]
    NotLoopback,
    /// No environment has the id.
    #[error("no environment of the project has this id")]
    UnknownEnvironment,
}

/// `origin` in its canonical form when the environment `id` of `environments` (the config in
/// effect) does not list it yet; `None` when it does. Exactly that origin: another loopback
/// name of the port is another origin.
///
/// # Errors
///
/// [`AddError`]: the origin is no bare loopback `http` origin, or no environment has the id.
pub fn origin_to_add(
    environments: &[Environment],
    id: &str,
    origin: &str,
) -> Result<Option<String>, AddError> {
    let canonical = Some(origin)
        .filter(|origin| rb_config::is_http_origin(origin))
        .and_then(origin_of)
        .filter(|canonical| is_loopback(canonical))
        .ok_or(AddError::NotLoopback)?;
    let environment = environments
        .iter()
        .find(|env| env.id == id)
        .ok_or(AddError::UnknownEnvironment)?;
    let listed = environment
        .asset_origins
        .iter()
        .any(|known| origin_of(known).as_deref() == Some(canonical.as_str()));
    Ok((!listed).then_some(canonical))
}

/// The entry of the environment `id` in a config file's JSON, if the file has one.
fn entry<'a>(document: &'a Value, id: &str) -> Option<&'a Value> {
    document
        .get(ENVIRONMENTS_KEY)?
        .as_array()?
        .iter()
        .find(|env| env.get("id").and_then(Value::as_str) == Some(id))
}

/// The file the environment `id`'s `assetOrigins` in effect comes from: `config.local.json`
/// when its entry sets the list (it replaces the other), else `config.json` when it has an entry
/// for the environment, else `config.local.json` when only it has one; `None` when neither does.
#[must_use]
pub fn target_file(project: &Value, local: Option<&Value>, id: &str) -> Option<ConfigFile> {
    let local_entry = local.and_then(|local| entry(local, id));
    if local_entry.is_some_and(|env| env.get(ASSET_ORIGINS).is_some()) {
        Some(ConfigFile::Local)
    } else if entry(project, id).is_some() {
        Some(ConfigFile::Project)
    } else {
        local_entry.map(|_| ConfigFile::Local)
    }
}

/// `document` (one config file's JSON) with `origin` appended to the `assetOrigins` of the
/// environment `id` (the list made when the entry has none); every other environment, field
/// and key as it was. `None` when the file has no entry for the environment or its list is no
/// list.
#[must_use]
pub fn with_file_origin(document: &Value, id: &str, origin: &str) -> Option<Value> {
    let mut out = document.clone();
    let env = out
        .get_mut(ENVIRONMENTS_KEY)?
        .as_array_mut()?
        .iter_mut()
        .find(|env| env.get("id").and_then(Value::as_str) == Some(id))?
        .as_object_mut()?;
    env.entry(ASSET_ORIGINS)
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()?
        .push(Value::String(origin.to_owned()));
    Some(out)
}

/// What the button adds: an origin to an environment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    /// The environment's id.
    pub environment: String,
    /// The loopback origin.
    pub origin: String,
}

impl Offer {
    /// The command's arguments.
    #[must_use]
    pub fn args(&self) -> Value {
        json!({ "environment": self.environment, "origin": self.origin })
    }

    /// The offer `args` name - both `origin` and `environment` - or, without either, `pending`
    /// (the oldest offer that waits).
    ///
    /// # Errors
    ///
    /// Only one of them, one that is not a string, or neither and no offer waits.
    pub fn from_args(args: &Value, pending: Option<Offer>) -> Result<Self, CommandError> {
        let field = |name: &str| args.get(name).filter(|value| !value.is_null());
        match (field("origin"), field("environment")) {
            (None, None) => pending.ok_or_else(|| {
                CommandError::Failed("the portal has offered no asset origin to add".to_owned())
            }),
            (Some(Value::String(origin)), Some(Value::String(environment))) => Ok(Self {
                environment: environment.clone(),
                origin: origin.clone(),
            }),
            _ => Err(CommandError::InvalidArgs(
                "origin and environment are strings, given together".to_owned(),
            )),
        }
    }
}

/// What was offered for the page origin shown, and the offers that wait for a click, oldest
/// first. Another page origin (another environment shown) and a start of the portal offer again.
#[derive(Debug, Default)]
pub struct OfferBook {
    page: Option<String>,
    offered: Vec<String>,
    pending: VecDeque<Offer>,
}

impl OfferBook {
    /// An empty book, as at a start of the portal.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            page: None,
            offered: Vec::new(),
            pending: VecDeque::new(),
        }
    }

    /// Whether `origin`, named by a page of `page_origin`, is to be offered now: not offered
    /// since that page origin was shown. A page of another origin forgets what was offered.
    pub fn note(&mut self, page_origin: &str, origin: &str) -> bool {
        if self.page.as_deref() != Some(page_origin) {
            self.page = Some(page_origin.to_owned());
            self.offered.clear();
        }
        if self.offered.len() >= MAX_OFFERED || self.offered.iter().any(|known| known == origin) {
            return false;
        }
        self.offered.push(origin.to_owned());
        true
    }

    /// `offer` waits for a click (once; the oldest goes when too many wait).
    pub fn wait(&mut self, offer: Offer) {
        if self.pending.contains(&offer) {
            return;
        }
        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(offer);
    }

    /// The oldest offer that waits.
    #[must_use]
    pub fn oldest(&self) -> Option<Offer> {
        self.pending.front().cloned()
    }

    /// `offer` was taken: it waits no more.
    pub fn taken(&mut self, offer: &Offer) {
        self.pending.retain(|waiting| waiting != offer);
    }

    /// Forgets everything (the portal started).
    pub fn clear(&mut self) {
        *self = Self::new();
    }
}

/// Refuses the command `id` to an MCP client when it is [`ADD_ASSET_ORIGIN`]: it writes the
/// project config, and that is a person's decision.
///
/// # Errors
///
/// [`CommandError::Failed`] for [`ADD_ASSET_ORIGIN`].
pub fn refuse_mcp(id: &str) -> Result<(), CommandError> {
    if id == ADD_ASSET_ORIGIN {
        return Err(CommandError::Failed(format!(
            "{ADD_ASSET_ORIGIN} is {ADD_OMITTED}; MCP cannot run it"
        )));
    }
    Ok(())
}

/// Where an offer goes: the page's origin and the unmapped loopback origin.
pub type Sink = Arc<dyn Fn(&str, &str) + Send + Sync>;

/// The running portal's sink; none while it does not run.
static SINK: Mutex<Option<Sink>> = Mutex::new(None);

/// Sets (or, with `None`, clears) where offers go.
pub fn set_sink(sink: Option<Sink>) {
    *lock(&SINK) = sink;
}

/// The ports of rBrowser's own listeners on this machine - the MCP server, the CDP proxy - asked
/// each time an origin is to be offered (they move with their settings).
pub type OwnPorts = Arc<dyn Fn() -> Vec<u16> + Send + Sync>;

/// Where the own listeners' ports come from; none while the portal does not run.
static OWN_PORTS: Mutex<Option<OwnPorts>> = Mutex::new(None);

/// Sets (or, with `None`, clears) where the own listeners' ports come from.
pub fn set_own_ports(ports: Option<OwnPorts>) {
    *lock(&OWN_PORTS) = ports;
}

/// The port of an `http` origin.
fn port_of(origin: &str) -> Option<u16> {
    Upstream::parse(origin).map(|upstream| upstream.port())
}

/// Offers `origin`, an unmapped loopback origin a body of the page `map` stands for names or a
/// device of it missed, to the sink with the page's origin (called outside every lock) - never
/// one on a port of rBrowser's own listeners: the portal's own (the map's portal origins), the
/// MCP server's and the CDP proxy's ([`set_own_ports`]). An asset origin is forwarded into the
/// LAN; none of those may ever be a click away (RB-090-72 review).
pub(super) fn offer(map: &OriginMap, origin: &str) {
    let Some(port) = port_of(origin) else {
        return;
    };
    let own_ports = lock(&OWN_PORTS).clone();
    let own = map.portals().filter_map(port_of).any(|own| own == port)
        || own_ports.is_some_and(|ports| ports().contains(&port));
    if own {
        tracing::debug!(
            %origin,
            "an origin on the port of one of rBrowser's own listeners is never offered"
        );
        return;
    }
    let sink = lock(&SINK).clone();
    if let Some(sink) = sink {
        sink(map.main_upstream(), origin);
    }
}

#[cfg(test)]
#[path = "offer_tests.rs"]
mod tests;
