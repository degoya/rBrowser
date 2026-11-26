//! `view_metrics` (RB-060-10), run by the host (`rb_mcp::Backend::host_tool`): per view of the
//! target its Web Vitals with their rating and the element behind them, its CPU share and
//! memory, and its throttling (RB-020-09). What is missing says why (`na` with a code and a
//! reason; `settingsKey` when a setting switched it off). The selectors come from the page's
//! agent - page data, as the other page reads of MCP.

use rb_config::EngineKind;
use rb_engine::ViewId;
use rb_ipc::{ResourceReading, Unavailable, ViewThrottle, VitalReading};
use rb_mcp::ToolFailure;
use rb_sync::VitalMetric;
use serde_json::{Map, Value, json};

use super::rating::selector_of;
use super::store::{Options, Store, ViewFacts};
use crate::control::Core;

/// The host tool's name.
pub const VIEW_METRICS_TOOL: &str = "view_metrics";

/// A view as the answer names it.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewRow {
    /// The view.
    pub view: ViewId,
    /// The breakpoint it shows.
    pub breakpoint: Option<String>,
    /// Its engine.
    pub engine: EngineKind,
    /// Its URL.
    pub url: String,
    /// Its throttling; `None` while visible.
    pub throttle: Option<ViewThrottle>,
}

/// The `views` argument: the view ids `rb-mcp` resolved the target to; `None` for every view.
///
/// # Errors
///
/// `views` is not a list of view ids.
pub fn wanted(args: &Value) -> Result<Option<Vec<ViewId>>, String> {
    match args.get("views") {
        None | Some(Value::Null) => Ok(None),
        Some(views) => serde_json::from_value::<Vec<u32>>(views.clone())
            .map(|ids| Some(ids.into_iter().map(ViewId).collect()))
            .map_err(|_| "`views` must be a list of view ids".to_owned()),
    }
}

fn na(code: &str, reason: &str, settings_key: Option<&str>) -> Value {
    let mut fields = Map::new();
    fields.insert("na".to_owned(), json!(code));
    fields.insert("reason".to_owned(), json!(reason));
    if let Some(key) = settings_key {
        fields.insert("settingsKey".to_owned(), json!(key));
    }
    Value::Object(fields)
}

fn unavailable(reason: Unavailable, resources: bool) -> Value {
    match reason {
        Unavailable::Pending if resources => na("pending", "not read yet", None),
        Unavailable::Pending => na(
            "pending",
            "not measured yet: no paint or no interaction so far",
            None,
        ),
        Unavailable::Unsupported => na("unsupported", "the view's engine cannot measure it", None),
        Unavailable::Soft => na(
            "soft",
            "a soft navigation's segment measures only CLS and INP",
            None,
        ),
        Unavailable::Engine if resources => {
            na("engine", "only Chromium views report CPU and memory", None)
        }
        Unavailable::Engine => na("engine", "the view's engine reports no Web Vitals", None),
    }
}

fn vital(reading: &VitalReading, store: &Store, view: ViewId) -> Value {
    let (Some(value), Some(rating)) = (reading.value, reading.rating) else {
        return unavailable(reading.unavailable.unwrap_or(Unavailable::Pending), false);
    };
    let mut fields = Map::new();
    fields.insert("value".to_owned(), json!(value));
    if reading.metric != VitalMetric::Cls {
        fields.insert("unit".to_owned(), json!("ms"));
    }
    fields.insert("rating".to_owned(), json!(rating));
    let report = store.report(view);
    match reading.metric {
        VitalMetric::Cls => {
            if let Some(cls) = report.and_then(|report| report.cls.as_ref()) {
                fields.insert("sources".to_owned(), json!(cls.sources));
            }
        }
        VitalMetric::Inp => {
            if let Some(inp) = report.and_then(|report| report.inp.as_ref()) {
                fields.insert("interactions".to_owned(), json!(inp.interactions));
            }
            if let Some(target) = report.and_then(|report| selector_of(report, reading.metric)) {
                fields.insert("target".to_owned(), json!(target));
            }
        }
        VitalMetric::Lcp => {
            if let Some(element) = report.and_then(|report| selector_of(report, reading.metric)) {
                fields.insert("element".to_owned(), json!(element));
            }
        }
        VitalMetric::Fcp | VitalMetric::Ttfb => {}
    }
    Value::Object(fields)
}

fn resources(reading: Option<&ResourceReading>) -> Value {
    let Some(reading) = reading else {
        return na("off", "switched off", Some("resources.enabled"));
    };
    if let Some(reason) = reading.unavailable {
        return unavailable(reason, true);
    }
    json!({
        "cpuPercent": reading.cpu_percent,
        "memoryMb": reading.memory_mb,
        "cpuWarn": reading.cpu_warn,
        "ramWarn": reading.ram_warn,
    })
}

/// The answer for `rows`.
#[must_use]
pub fn answer(store: &Store, rows: &[ViewRow], options: &Options) -> Value {
    let facts: Vec<ViewFacts> = rows
        .iter()
        .map(|row| ViewFacts {
            view: row.view,
            engine: row.engine,
            throttle: row.throttle,
        })
        .collect();
    let metrics = store.metrics(&facts, options);
    let views: Vec<Value> = rows
        .iter()
        .zip(metrics)
        .map(|(row, metrics)| {
            let vitals: Map<String, Value> = VitalMetric::ALL
                .iter()
                .map(|metric| {
                    let value = metrics
                        .vitals
                        .iter()
                        .find(|reading| reading.metric == *metric)
                        .map_or_else(
                            || na("off", "switched off", Some("vitals.enabled")),
                            |reading| vital(reading, store, row.view),
                        );
                    (metric.id().to_owned(), value)
                })
                .collect();
            let segment = store.report(row.view).map(|report| report.segment);
            json!({
                "view": row.view.0,
                "breakpoint": row.breakpoint,
                "engine": row.engine,
                "url": row.url,
                "segment": segment,
                "soft": metrics.soft,
                "vitals": vitals,
                "resources": resources(metrics.resources.as_ref()),
                "throttle": row.throttle.map_or_else(
                    || json!({ "state": "visible" }),
                    |throttle| json!(throttle),
                ),
            })
        })
        .collect();
    json!({ "views": views })
}

/// Runs `view_metrics` in `core`'s window.
///
/// # Errors
///
/// The arguments are malformed, or the window has no metrics yet.
pub fn view_metrics(core: &Core, args: &Value) -> Result<Value, ToolFailure> {
    let wanted = wanted(args).map_err(|message| ToolFailure::new("args.invalid", message))?;
    let host = core.host();
    let vitals = super::for_host(host)
        .ok_or_else(|| ToolFailure::new("metrics.not_ready", "the window measures nothing yet"))?;
    let rows: Vec<ViewRow> = rb_engine::ViewHost::views(host.as_ref())
        .into_iter()
        .filter(|info| wanted.as_ref().is_none_or(|ids| ids.contains(&info.id)))
        .map(|info| ViewRow {
            view: info.id,
            breakpoint: info.breakpoint,
            engine: info.engine,
            url: info.url,
            throttle: super::throttle::throttle_of(host, info.id),
        })
        .collect();
    let options = Options::of(&core.user_settings());
    Ok(vitals.with_store(|store| answer(store, &rows, &options)))
}

#[cfg(test)]
#[path = "mcp_tests.rs"]
mod tests;
