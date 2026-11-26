//! What a missing format becomes (RB-099-29): the notice in the status bar - the format, the
//! file, that WebM (VP9, AV1) and Opus play, and a button to the handbook's page on media
//! formats - and the Info finding of the rule `media.unsupported-format` with the page (its
//! secrets masked, as the other findings name it), the source (without credentials, query and
//! fragment) and the element. Pure: the task in `mod.rs` sends them.

use std::collections::BTreeMap;

use rb_ipc::{Notification, NotificationLevel, StatusAction};
use rb_plugin_types::{Finding, Severity};
use rb_sync::MediaMessage;
use serde_json::json;

use super::classify::Missing;
use crate::checks::reported::Keep;
use crate::open_link::OPEN_LINK_COMMAND;

/// The rule group of the findings: the core reports them itself, as `api-client` does.
pub const PLUGIN: &str = "media";
/// The rule: a media file in a format this build does not play.
pub const RULE: &str = "unsupported-format";
/// The notice; `{source}` (the file), `{format}`.
pub const NOTICE_KEY: &str = "media.notice.unsupported";
/// The notice's button.
pub const ACTION_KEY: &str = "media.notice.formats";
/// The finding's text; `{source}` (the address), `{format}`.
pub const FINDING_KEY: &str = "media.finding.unsupported";
/// The handbook's page on media formats (below `open_link::HANDBOOK_URL`).
pub const WIKI_URL: &str = "https://github.com/degoya/rBrowser/wiki/media-formats";
/// How the findings are published: in the panel only, never as a run under the project - they
/// come from browsing, and a written run would prune the person's check runs.
pub const KEEP: Keep = Keep::ShowOnly;
/// The longest source a finding names, in characters.
pub const MAX_SOURCE: usize = 300;

/// `src` without user info, query and fragment, cut to [`MAX_SOURCE`].
#[must_use]
pub fn source_label(src: &str) -> String {
    let bare = rb_util::url::without_user_info(src);
    let bare = bare.split(['?', '#']).next().unwrap_or_default();
    rb_util::text::cut(bare, MAX_SOURCE)
}

/// The file `src` names: its path's last segment, else its host, else the label.
#[must_use]
pub fn file_name(src: &str) -> String {
    let label = source_label(src);
    let path = match label.split_once("://") {
        Some((_, rest)) => rest.find('/').map_or("", |at| &rest[at..]),
        None if label.starts_with('/') => label.as_str(),
        None => return label,
    };
    if let Some(segment) = path.rsplit('/').find(|segment| !segment.is_empty()) {
        return segment.to_owned();
    }
    rb_util::url::authority_of(&label).map_or_else(|| label.clone(), str::to_owned)
}

/// The notice for `missing` of `src`.
#[must_use]
pub fn notification(missing: &Missing, src: &str) -> Notification {
    Notification {
        level: NotificationLevel::Info,
        message_key: NOTICE_KEY.to_owned(),
        args: BTreeMap::from([
            ("format".to_owned(), missing.format.label()),
            ("source".to_owned(), file_name(src)),
        ]),
        settings_key: None,
    }
}

/// The notice's button: the handbook's page on media formats (`app.open_link`).
#[must_use]
pub fn action() -> StatusAction {
    StatusAction {
        command: OPEN_LINK_COMMAND.to_owned(),
        label_key: ACTION_KEY.to_owned(),
        label_args: BTreeMap::new(),
        args: json!({ "url": WIKI_URL }),
    }
}

/// The finding of `missing` in `view` on `page`, for the element `media` names.
#[must_use]
pub fn finding(view: u32, page: &str, media: &MediaMessage, missing: &Missing) -> Finding {
    let source = source_label(&media.src);
    let data = json!({
        "format": missing.format.id(),
        "evidence": missing.evidence.name(),
        "tag": media.tag,
        "type": media.mime_type,
    });
    Finding {
        rule: RULE.to_owned(),
        severity: Severity::Info,
        message: FINDING_KEY.to_owned(),
        params: vec![
            ("format".to_owned(), missing.format.label()),
            ("source".to_owned(), source),
        ],
        view: Some(view),
        url: Some(rb_util::mask::mask_url(page)),
        selector: Some(media.path.clone()),
        path: Some(media.path.clone()),
        data: Some(data.to_string()),
    }
}

/// The finding texts of `plugin`: the core's own for [`PLUGIN`], a plugin's catalogue else.
#[must_use]
pub fn texts(plugin: &str) -> BTreeMap<String, String> {
    if plugin == PLUGIN {
        return BTreeMap::from([(FINDING_KEY.to_owned(), rb_i18n::t(FINDING_KEY))]);
    }
    rb_i18n::plugin_texts(plugin)
}

#[cfg(test)]
#[path = "notice_tests.rs"]
mod tests;
