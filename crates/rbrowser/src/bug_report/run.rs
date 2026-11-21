//! Gathering a bug report from the window (RB-093-04): the views' screenshots through the
//! capture (`crate::capture::screenshot`, password fields blacked out, into the report's
//! `screens/`), the overview the same way, each view's state and emulation, the environment, the
//! window's buffers (`crate::plugins::console::current`) and the cookie values of the Chromium
//! views' sessions - read only to be masked, never written - then [`super::compose`], the
//! person's look at the texts ([`super::preview`], `report.preview`), `report.md` and
//! `report.zip`. The folder is the project's or the person's `report.folder`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rb_config::{BugReportConfig, EngineKind, ProjectConfig, ReportPart};
use rb_engine::{Target, ViewHost as _, ViewId, ViewInfo};
use serde_json::{Value, json};

use super::markdown::{Image, Report};
use super::mask::Masker;
use super::preview::{self, Draft};
use super::{Gathered, RawView, bundle, compose, markdown, report_text, secrets};
use crate::capture::{CaptureMode, ScreenshotRequest, ShotAnswer};
use crate::control::Core;
use crate::host::Host;

/// A written report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    /// Its folder below `.rbrowser/out/reports/`.
    pub folder: PathBuf,
    /// `report.md`.
    pub markdown: PathBuf,
    /// `report.zip`.
    pub zip: PathBuf,
    /// `report.md`'s text, masked: what "Copy Markdown" copies.
    pub text: String,
}

/// Why no report was written.
#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    /// The arguments do not fit.
    #[error("{0}")]
    Invalid(String),
    /// No project, no view, or the disk.
    #[error("{0}")]
    Failed(String),
    /// The person cancelled it in the preview (or left it): nothing was saved.
    #[error("the bug report was cancelled; nothing was saved")]
    Cancelled,
}

/// Makes the report of the views `target` names (every view when `None`) with `note`.
///
/// # Errors
///
/// No open project, an unknown view, no view, the disk, or the person's cancel in the preview.
pub async fn create(
    core: &Arc<Core>,
    target: Option<Vec<ViewId>>,
    note: Option<String>,
) -> Result<Written, ReportError> {
    let host = core.host().clone();
    let root = host
        .project_root()
        .ok_or_else(|| ReportError::Failed(rb_i18n::t("report.error.no_project")))?;
    let ids = match target {
        None => host
            .target_views(Some(Target::All))
            .map_err(|error| ReportError::Failed(error.to_string()))?,
        Some(ids) => ids,
    };
    let infos: Vec<ViewInfo> = host
        .views()
        .into_iter()
        .filter(|info| ids.contains(&info.id))
        .collect();
    if let Some(unknown) = ids
        .iter()
        .find(|id| !infos.iter().any(|info| info.id == **id))
    {
        return Err(ReportError::Invalid(format!("there is no view {unknown}")));
    }
    if infos.is_empty() {
        return Err(ReportError::Failed(rb_i18n::t("report.error.no_views")));
    }
    let config = host.config();
    let settings = config.report.clone();
    let annotate = settings.annotate;
    let person = core.user_settings().report;
    let created = crate::local_time::stamp_now().date_time();
    let folder =
        bundle::new_report_folder(&root, person.chosen_folder(), &created).map_err(write_failed)?;
    let ids: Vec<ViewId> = infos.iter().map(|info| info.id).collect();
    let (images, overview) = shoot(core, &ids, &folder, &settings).await;
    let views: Vec<RawView> = infos.iter().map(|info| raw_view(&host, info)).collect();
    let cookies = cookie_values(&host, &infos).await;
    let gathered = Gathered {
        created,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        environment: environment_of(&config, host.shell_state().environment.as_deref()),
        note,
        views,
        feed: crate::plugins::console::current(),
        images,
        overview,
        settings,
    };
    let masker = Masker::new(secrets(cookies, &gathered.views));
    let report = compose(&gathered, &masker);
    let text = markdown(&report, &masker, &report_text);
    if person.preview && !preview::ask(core, draft_of(&folder, &text, &report, annotate)).await {
        bundle::discard(&folder);
        return Err(ReportError::Cancelled);
    }
    let markdown = bundle::write_markdown(&folder, &text).map_err(write_failed)?;
    let zip = bundle::write_zip(&folder).map_err(write_failed)?;
    Ok(Written {
        folder,
        markdown,
        zip,
        text,
    })
}

/// What the preview shows of `report`: its folder, `text` as it would be saved, the images
/// taken (a failed capture is none) and whether the editor opens after saving.
pub fn draft_of(folder: &Path, text: &str, report: &Report, annotate: bool) -> Draft {
    let images = report
        .images
        .iter()
        .chain(report.overview.as_ref())
        .filter(|image| image.path.is_some())
        .count();
    Draft {
        folder: folder.display().to_string(),
        markdown: text.to_owned(),
        images: u32::try_from(images).unwrap_or(u32::MAX),
        annotate,
    }
}

fn write_failed(error: std::io::Error) -> ReportError {
    ReportError::Failed(rb_i18n::t_with(
        "report.error.write",
        &[("reason", &error.to_string())],
    ))
}

/// The screenshots of `ids` and the overview, as `settings` asks, into `folder`'s `screens/`;
/// a failed capture becomes each image's reason.
async fn shoot(
    core: &Arc<Core>,
    ids: &[ViewId],
    folder: &Path,
    settings: &BugReportConfig,
) -> (Vec<Image>, Option<Image>) {
    let screens = folder.join(bundle::SCREENS);
    let mut images = Vec::new();
    if settings.has(ReportPart::Screenshots) {
        images = match crate::capture::screenshot(core, &request(ids, &screens, false)).await {
            Ok(answer) => answer
                .shots
                .iter()
                .map(|shot| image_of(folder, shot))
                .collect(),
            Err(error) => ids
                .iter()
                .map(|id| Image {
                    view: Some(id.0),
                    path: None,
                    error: Some(error.to_string()),
                })
                .collect(),
        };
    }
    let overview = if settings.has(ReportPart::Overview) {
        match crate::capture::screenshot(core, &request(ids, &screens, true)).await {
            Ok(answer) => answer.overview.as_ref().map(|shot| image_of(folder, shot)),
            Err(error) => Some(Image {
                view: None,
                path: None,
                error: Some(error.to_string()),
            }),
        }
    } else {
        None
    };
    (images, overview)
}

/// The capture of a report: the viewport of `ids` (or their overview) into `screens`, at once,
/// with the password fields blacked out.
pub fn request(ids: &[ViewId], screens: &Path, overview: bool) -> ScreenshotRequest {
    ScreenshotRequest {
        target: None,
        mode: CaptureMode::Viewport,
        selector: None,
        padding: 0,
        overview,
        delay: Duration::ZERO,
        save: true,
        views: Some(ids.to_vec()),
        folder: Some(screens.to_path_buf()),
        clean: rb_visual::CleanOverrides::default(),
        at: None,
        mask_passwords: true,
    }
}

/// The report's image of `shot`: its path relative to `folder`.
pub fn image_of(folder: &Path, shot: &ShotAnswer) -> Image {
    Image {
        view: shot.view.map(|id| id.0),
        path: shot
            .path
            .as_deref()
            .and_then(|path| bundle::relative(folder, Path::new(path))),
        error: shot.error.clone(),
    }
}

/// The environment `id` with its address in `config`; none without an environment.
pub fn environment_of(config: &ProjectConfig, id: Option<&str>) -> Option<(String, String)> {
    let id = id?;
    let url = config
        .environments
        .iter()
        .find(|environment| environment.id == id)
        .map(|environment| environment.url.clone())
        .unwrap_or_default();
    Some((id.to_owned(), url))
}

fn raw_view(host: &Host, info: &ViewInfo) -> RawView {
    RawView {
        id: info.id.0,
        label: info.label.clone(),
        breakpoint: info.breakpoint.clone(),
        width: info.width,
        height: info.height,
        dpr: info.dpr,
        engine: info.engine.name().to_owned(),
        url: info.url.clone(),
        title: info.title.clone(),
        emulation: host
            .emulation(info.id)
            .and_then(|emulation| serde_json::to_value(emulation).ok())
            .unwrap_or(Value::Null),
    }
}

/// The cookie values of the Chromium views' sessions (one read per session): secrets to mask,
/// kept in memory for this report only. A session that cannot be read is masked by the rules.
async fn cookie_values(host: &Arc<Host>, infos: &[ViewInfo]) -> Vec<String> {
    let mut values = Vec::new();
    let mut sessions = BTreeSet::new();
    for info in infos {
        if info.engine != EngineKind::Chromium || !sessions.insert(info.session.clone()) {
            continue;
        }
        match host
            .cdp_send(info.id, crate::cookies::jars::READ_METHOD, json!({}))
            .await
        {
            Ok(answer) => values.extend(
                crate::cookies::model::cookies_of(&answer)
                    .into_iter()
                    .map(|cookie| cookie.value),
            ),
            Err(error) => tracing::debug!(%error, "a session's cookies are unknown to the report"),
        }
    }
    values
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
