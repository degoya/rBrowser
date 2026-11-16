//! The CDP mapping of the emulation extras (RB-080-10): geolocation, time zone and CPU
//! throttling, one command each, sent when they changed - or, after a reconnect, again; and the
//! geolocation permission, granted to the origin of a view with a position and to no other.
//! Offline rides on `Network.emulateNetworkConditions`, the print view on
//! `Emulation.setEmulatedMedia` ([`super::cdp`]). Pure; the host sends the calls.

use rb_engine::Emulation;
use rb_util::url::origin_of;
use serde_json::{Value, json};

use super::cdp::CdpCall;

/// The calls that bring the extras from `before` to `next`; with `full` (a new session) every
/// extra `next` sets goes again.
pub fn extras_calls(before: &Emulation, next: &Emulation, full: bool) -> Vec<CdpCall> {
    let mut calls = Vec::new();
    if (full && next.geolocation.is_some()) || next.geolocation != before.geolocation {
        calls.push(match next.geolocation {
            Some(position) => CdpCall::new(
                "Emulation.setGeolocationOverride",
                json!({
                    "latitude": position.latitude,
                    "longitude": position.longitude,
                    "accuracy": position.accuracy,
                }),
            ),
            None => CdpCall::new("Emulation.clearGeolocationOverride", json!({})),
        });
    }
    if (full && next.timezone.is_some()) || next.timezone != before.timezone {
        // An empty id gives the page the system's time zone back.
        calls.push(CdpCall::new(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": next.timezone.clone().unwrap_or_default() }),
        ));
    }
    if (full && next.cpu_throttling.is_some()) || next.cpu_throttling != before.cpu_throttling {
        calls.push(CdpCall::new(
            "Emulation.setCPUThrottlingRate",
            json!({ "rate": next.cpu_throttling.unwrap_or(1.0) }),
        ));
    }
    calls
}

/// What the geolocation permission of one view needs: the calls, and the origin it is granted
/// for afterwards.
#[derive(Debug, Default, PartialEq)]
pub struct PermissionPlan {
    /// The CDP calls, in order.
    pub calls: Vec<CdpCall>,
    /// The origin granted for the view after them.
    pub granted: Option<String>,
}

/// Brings the permission of a view from `granted` (the origin rBrowser granted for it) to what
/// the page at `url` needs: granted to its origin while the view has a `position`, without a
/// prompt; taken back (`prompt`, the default) from an origin the view leaves or no longer needs,
/// unless another view with a position `held` it. With `force` (a new session) the grant goes
/// again.
pub fn geolocation_permission(
    granted: Option<&str>,
    url: &str,
    position: bool,
    force: bool,
    held: &[&str],
) -> PermissionPlan {
    let wanted = position.then(|| origin_of(url)).flatten();
    let mut calls = Vec::new();
    if let Some(old) = granted
        && wanted.as_deref() != Some(old)
        && !held.contains(&old)
    {
        calls.push(CdpCall::new(
            "Browser.setPermission",
            json!({
                "permission": { "name": "geolocation" },
                "setting": "prompt",
                "origin": old,
            }),
        ));
    }
    if let Some(origin) = &wanted
        && (force || granted != Some(origin.as_str()))
    {
        calls.push(grant(origin));
    }
    PermissionPlan {
        calls,
        granted: wanted,
    }
}

/// Names the view's browser context (`browserContextId` of its target) in every permission
/// call: without it the browser takes its default context, which other sessions' views do not
/// live in. `None` leaves the calls as they are.
pub fn in_context(calls: &mut [CdpCall], context: Option<&str>) {
    let Some(context) = context else {
        return;
    };
    for call in calls {
        if let Some(params) = call.params.as_object_mut() {
            params.insert("browserContextId".to_owned(), Value::from(context));
        }
    }
}

fn grant(origin: &str) -> CdpCall {
    CdpCall::new(
        "Browser.grantPermissions",
        json!({ "permissions": ["geolocation"], "origin": origin }),
    )
}

#[cfg(test)]
#[path = "cdp_extras_tests.rs"]
mod tests;
