//! Media a view cannot play (RB-099-29). The pinned CEF build has no proprietary codecs: MP4
//! with H.264 and AAC, HEVC and QuickTime do not play; WebM (VP8, VP9, AV1), Opus, Vorbis, MP3
//! and FLAC do. The sync agent hears a media element's or a source's failure in every frame
//! (`agent/src/media.ts`, `rb_sync::MediaMessage`) - the simpler of the two ways: the CDP
//! `Media` domain knows no `<source>` the element skipped for its type (no player is made) and
//! names no element. The core decides whether a format is missing ([`classify`]), gives one
//! notice per page and format in the status bar with a button to the handbook's page on media
//! formats, and reports an Info finding of the rule group `media` ([`notice`], [`state`]), so
//! MCP's `checks_*` tools see it. No setting; the findings follow `checks.rules` and the ignore
//! patterns as every finding. Chromium views only: Firefox plays these formats itself.
//!
//! | File | What |
//! | --- | --- |
//! | `classify.rs` | the format a report misses, and why |
//! | `state.rs` | once per page and format; the findings so far |
//! | `notice.rs` | the notice, its button, the finding, the texts |

pub mod classify;
pub mod notice;
pub mod state;

use rb_engine::{ViewHost as _, ViewId};
use rb_sync::{AgentMessage, MediaMessage};
use tokio::sync::broadcast::error::RecvError;

use crate::control::Core;
use state::State;

/// Starts the listener of `core`'s agents.
pub fn on_start(core: &Core) {
    let Some(core) = core.arc() else {
        return;
    };
    let mut reports = core.host().sync_agents().subscribe();
    let weak = std::sync::Arc::downgrade(&core);
    core.runtime().spawn(async move {
        let mut state = State::default();
        loop {
            let report = match reports.recv().await {
                Ok(report) => report,
                Err(RecvError::Lagged(missed)) => {
                    tracing::debug!(missed, "agent reports dropped (media formats)");
                    continue;
                }
                Err(RecvError::Closed) => return,
            };
            let AgentMessage::Media(media) = &report.message else {
                continue;
            };
            let Some(core) = weak.upgrade() else {
                return;
            };
            on_media(&core, &mut state, report.view, media);
        }
    });
}

/// The page `media` was reported on: the top document's own address, a frame's view's.
fn page_of(core: &Core, view: ViewId, media: &MediaMessage) -> String {
    if media.top {
        return media.url.clone();
    }
    core.host()
        .views()
        .into_iter()
        .find(|info| info.id == view)
        .map_or_else(|| media.url.clone(), |info| info.url)
}

fn on_media(core: &Core, state: &mut State, view: ViewId, media: &MediaMessage) {
    let page = page_of(core, view, media);
    let Some(outcome) = state.on_media(view.0, &page, media) else {
        return;
    };
    // The host only: an address may carry a token.
    tracing::info!(
        %view,
        host = %rb_util::url::host_of(&page).unwrap_or_default(),
        "a media format this build does not play"
    );
    core.notify_with_action(outcome.notification, notice::action());
    // Shown only: browsing writes no check run and prunes none of the person's.
    crate::checks::reported::publish_with(
        core,
        notice::PLUGIN,
        outcome.findings,
        notice::texts,
        notice::KEEP,
    );
}
