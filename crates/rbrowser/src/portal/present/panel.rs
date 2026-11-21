//! What the Portal panel, the activity bar and MCP see of a presentation (RB-096-05). The panel
//! and the bar get the public address and the open link with its QR code (`getPortal` only); MCP
//! (`portal_present_status`, and `portal_devices` through the panel's state without the
//! pairing) gets the phase, the viewers and the link's time left - never the link, its token or
//! the address.

use std::sync::atomic::Ordering;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rb_ipc::{PortalPairing, PortalPresentation, PresentationDownload, PresentationPhase};
use serde_json::{Value, json};

use super::app::{DOWNLOADING, Phase, STATE};
use super::cloudflared;
use crate::cdp::lock;
use crate::control::Core;
use crate::portal::qr::QrMatrix;

/// The presentation for the panel; `with_link` adds the open link and its QR code.
#[must_use]
pub fn panel(core: &Core, with_link: bool) -> PortalPresentation {
    let user = core.user_settings();
    let settings = &user.portal.presentation;
    let mut presentation = PortalPresentation {
        enabled: settings.enabled,
        consented: settings.consented,
        max_viewers: u32::try_from(settings.viewer_limit()).unwrap_or(u32::MAX),
        ..PortalPresentation::default()
    };
    let running = lock(&STATE).as_ref().map(|running| {
        (
            running.server.clone(),
            running.public.origin(),
            running.phase,
            running.link.clone(),
            running.started_at,
        )
    });
    let Some((server, address, phase, link, started_at)) = running else {
        if DOWNLOADING.load(Ordering::Acquire) {
            presentation.phase = PresentationPhase::Downloading;
        } else if settings.enabled
            && cloudflared::locate_default(user.tools.cloudflared_path.as_deref()).is_none()
        {
            presentation.download = cloudflared::download().map(|offer| PresentationDownload {
                version: offer.version,
                size_mb: offer.size_mb,
                license: offer.license,
                from: offer.from,
            });
        }
        return presentation;
    };
    presentation.phase = match phase {
        Phase::Starting => PresentationPhase::Starting,
        Phase::Live => PresentationPhase::Live,
    };
    // The panel's only, like the link: no MCP answer names the public address.
    presentation.address = address.filter(|_| with_link);
    presentation.viewers = u32::try_from(server.devices().len()).unwrap_or(u32::MAX);
    presentation.started_at_ms = Some(epoch_ms(started_at));
    presentation.link = link
        .filter(|_| with_link && server.link_left(Instant::now()).is_some())
        .and_then(|(url, expires_at)| {
            let matrix = QrMatrix::encode(&url).ok()?;
            Some(PortalPairing {
                url,
                size: u32::try_from(matrix.size).ok()?,
                rows: matrix.rows,
                expires_at_ms: epoch_ms(expires_at),
            })
        });
    presentation
}

/// `portal_present_status`: running, phase, viewers and their limit, whether a link is open and
/// its seconds left, the start - nothing that lets anyone join.
#[must_use]
pub fn status(core: &Core) -> Value {
    let max_viewers = core.user_settings().portal.presentation.viewer_limit();
    let running = lock(&STATE)
        .as_ref()
        .map(|running| (running.server.clone(), running.phase, running.started_at));
    let Some((server, phase, started_at)) = running else {
        return json!({
            "running": false,
            "phase": if DOWNLOADING.load(Ordering::Acquire) { "downloading" } else { "off" },
            "maxViewers": max_viewers,
        });
    };
    let left = server.link_left(Instant::now());
    json!({
        "running": true,
        "phase": match phase {
            Phase::Starting => "starting",
            Phase::Live => "live",
        },
        "viewers": server.devices().len(),
        "maxViewers": max_viewers,
        "linkOpen": left.is_some(),
        "linkExpiresInSec": left.map(|left| left.as_secs()),
        "startedAtMs": epoch_ms(started_at),
    })
}

fn epoch_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH).map_or(0, |since| {
        u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
    })
}
