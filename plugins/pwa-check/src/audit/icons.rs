//! The manifest's icons: one of 192 and one of 512 px declared, each icon reachable (200), an
//! image of the size its `sizes` claims and the format its `type` names. An icon URL is
//! requested once however many entries name it; a `data:` icon is not requested.

use rb_plugin_api::PluginHost;
use serde::Serialize;

use crate::fetch::{self, ACCEPT_IMAGE, Budget, Fetched};
use crate::image::{Image, sniff};
use crate::manifest::{Icon, Manifest};
use crate::rules::{
    FETCH_OTHER_ORIGIN, ICONS_NOT_IMAGE, ICONS_SIZE_MISMATCH, ICONS_TYPE_MISMATCH,
    ICONS_UNREACHABLE, MANIFEST_ICON_SIZE, Place, Sink,
};
use crate::url::Url;

/// The sizes an installable app needs.
pub const REQUIRED_SIZES: [u32; 2] = [192, 512];
/// The most icons requested in one check.
pub const MAX_FETCHED: usize = 20;

/// One icon as the report shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconReport {
    pub index: usize,
    pub src: String,
    pub url: Option<String>,
    pub sizes: String,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub purpose: Vec<String>,
    /// `ok`, `unreachable`, `not_image`, `other_origin`, `skipped` (a `data:` icon, or past
    /// [`MAX_FETCHED`] or the time budget).
    pub state: String,
    pub status: Option<u16>,
    /// The format sniffed (`image/png`).
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub bytes: Option<usize>,
}

/// Whether some icon of the manifest is declared for `size` x `size` (or scalable).
fn declares(manifest: &Manifest, size: u32) -> bool {
    manifest
        .icons
        .iter()
        .any(|icon| icon.sizes.any || icon.sizes.listed.contains(&(size, size)))
}

/// What one requested icon came back as.
#[derive(Clone)]
enum Answer {
    Image {
        status: u16,
        image: Image,
        bytes: usize,
    },
    NotImage {
        status: u16,
    },
    Status(u16),
    OtherOrigin(String),
    Failed(String),
}

fn request(host: &mut dyn PluginHost, budget: &Budget, page: &Url, url: &Url) -> Answer {
    match fetch::get(host, budget, page, url, ACCEPT_IMAGE) {
        Fetched::Ok(response) if response.status == 200 => match sniff(&response.body) {
            Some(image) => Answer::Image {
                status: 200,
                image,
                bytes: response.body.len(),
            },
            None => Answer::NotImage { status: 200 },
        },
        Fetched::Ok(response) => Answer::Status(response.status),
        Fetched::OtherOrigin(url) => Answer::OtherOrigin(url),
        Fetched::Failed(code) => Answer::Failed(code),
    }
}

fn size_text(image: &Image) -> String {
    image
        .size
        .map_or_else(|| "any".to_owned(), |(w, h)| format!("{w}x{h}"))
}

/// The findings of one answered icon, and its report filled in.
fn judge(icon: &Icon, answer: &Answer, report: &mut IconReport, sink: &mut Sink<'_>) {
    let place = Place::Icon(icon.index);
    let src = ("src", icon.src.clone());
    match answer {
        Answer::Image {
            status,
            image,
            bytes,
        } => {
            report.state = "ok".to_owned();
            report.status = Some(*status);
            report.format = Some(image.format.mime().to_owned());
            report.width = image.size.map(|(w, _)| w);
            report.height = image.size.map(|(_, h)| h);
            report.bytes = Some(*bytes);
            if !icon.sizes_text.is_empty() && !image.fits(&icon.sizes) {
                let actual = ("actual", size_text(image));
                let declared = ("declared", icon.sizes_text.clone());
                sink.emit(
                    ICONS_SIZE_MISMATCH,
                    place.clone(),
                    &[src.clone(), declared, actual],
                );
            }
            if let Some(kind) = &icon.kind
                && !image.format.matches(kind)
            {
                let actual = ("actual", image.format.mime().to_owned());
                sink.emit(
                    ICONS_TYPE_MISMATCH,
                    place,
                    &[src, ("declared", kind.clone()), actual],
                );
            }
        }
        Answer::NotImage { status } => {
            report.state = "not_image".to_owned();
            report.status = Some(*status);
            sink.emit(ICONS_NOT_IMAGE, place, &[src]);
        }
        Answer::Status(status) => {
            report.state = "unreachable".to_owned();
            report.status = Some(*status);
            sink.emit(
                ICONS_UNREACHABLE,
                place,
                &[src, ("status", status.to_string())],
            );
        }
        Answer::OtherOrigin(url) => {
            report.state = "other_origin".to_owned();
            sink.emit(
                FETCH_OTHER_ORIGIN,
                place,
                &[("what", "icon".to_owned()), ("url", url.clone())],
            );
        }
        Answer::Failed(code) if code == "time" => report.state = "skipped".to_owned(),
        Answer::Failed(code) => {
            report.state = "unreachable".to_owned();
            sink.emit(ICONS_UNREACHABLE, place, &[src, ("status", code.clone())]);
        }
    }
}

/// The icon rules of `manifest` for the page at `page`.
pub fn check(
    host: &mut dyn PluginHost,
    budget: &Budget,
    page: &Url,
    manifest: &Manifest,
    sink: &mut Sink<'_>,
) -> Vec<IconReport> {
    for size in REQUIRED_SIZES {
        if !declares(manifest, size) {
            let px = ("size", size.to_string());
            sink.emit(MANIFEST_ICON_SIZE, Place::RequiredIcon(size), &[px]);
        }
    }
    let mut answers: Vec<(Url, Answer)> = Vec::new();
    let mut reports = Vec::new();
    for icon in &manifest.icons {
        let mut report = IconReport {
            index: icon.index,
            src: icon.src.clone(),
            url: icon.url.as_ref().map(ToString::to_string),
            sizes: icon.sizes_text.clone(),
            kind: icon.kind.clone(),
            purpose: icon.purpose.clone(),
            state: "skipped".to_owned(),
            ..IconReport::default()
        };
        if let Some(url) = &icon.url {
            let known = answers
                .iter()
                .find(|(seen, _)| seen == url)
                .map(|(_, a)| a.clone());
            let answer = match known {
                Some(answer) => Some(answer),
                None if answers.len() < MAX_FETCHED => {
                    let answer = request(host, budget, page, url);
                    answers.push((url.clone(), answer.clone()));
                    Some(answer)
                }
                None => None,
            };
            if let Some(answer) = answer {
                judge(icon, &answer, &mut report, sink);
            }
        }
        reports.push(report);
    }
    reports
}

#[cfg(test)]
#[path = "icons_tests.rs"]
mod tests;
