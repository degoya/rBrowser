//! The resources of a view (RB-060-10), from CDP `Performance.getMetrics` of its page: the CPU
//! share is the part of the interval the page's main thread was busy (`TaskDuration` over
//! `Timestamp`), the memory its JS heap (`JSHeapTotalSize`). Both are the page's own, so views
//! that share a renderer process are not counted twice. Above `resources.ramWarnMb` a view
//! warns at once; above `resources.cpuWarnPercent` only after [`CPU_HOLD`]. Each warning's
//! start and end is a [`Change`] - the log entry.

use std::time::{Duration, Instant};

use rb_ipc::ResourceReading;
use serde_json::Value;

/// How long the CPU share must stay above the limit before the view warns.
pub const CPU_HOLD: Duration = Duration::from_secs(5);
const MIB: f64 = 1_048_576.0;

/// The page metrics read here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageMetrics {
    /// `Timestamp`, s.
    pub timestamp_s: f64,
    /// `TaskDuration`: the main thread's busy time so far, s.
    pub task_duration_s: f64,
    /// `JSHeapTotalSize`, bytes.
    pub js_heap_bytes: f64,
}

/// The metrics of a `Performance.getMetrics` answer; `None` when one is missing (the domain is
/// not enabled yet).
#[must_use]
pub fn parse_metrics(answer: &Value) -> Option<PageMetrics> {
    let list = answer.get("metrics")?.as_array()?;
    let find = |name: &str| {
        list.iter()
            .find(|metric| metric.get("name").and_then(Value::as_str) == Some(name))
            .and_then(|metric| metric.get("value"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite())
    };
    Some(PageMetrics {
        timestamp_s: find("Timestamp")?,
        task_duration_s: find("TaskDuration")?,
        js_heap_bytes: find("JSHeapTotalSize")?,
    })
}

/// The warning limits (`resources.*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceLimits {
    /// `resources.ramWarnMb`.
    pub ram_warn_mb: u32,
    /// `resources.cpuWarnPercent`.
    pub cpu_warn_percent: u32,
}

/// A warning started or ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Change {
    /// The CPU share stayed above the limit; the share now.
    CpuWarn(f64),
    /// The CPU share is below the limit again.
    CpuCalm,
    /// The memory is above the limit; the MB now.
    RamWarn(f64),
    /// The memory is below the limit again.
    RamCalm,
}

fn one_decimal(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

/// One view's readings over time.
#[derive(Clone, Debug, Default)]
pub struct ViewResources {
    last: Option<PageMetrics>,
    busy_since: Option<Instant>,
    cpu_warn: bool,
    ram_warn: bool,
}

impl ViewResources {
    /// Takes one sample at `now`: the reading and the warnings that started or ended.
    pub fn sample(
        &mut self,
        metrics: PageMetrics,
        now: Instant,
        limits: ResourceLimits,
    ) -> (ResourceReading, Vec<Change>) {
        let cpu = self.last.and_then(|last| {
            let elapsed = metrics.timestamp_s - last.timestamp_s;
            let busy = metrics.task_duration_s - last.task_duration_s;
            (elapsed > 0.0 && busy >= 0.0)
                .then(|| one_decimal((busy / elapsed * 100.0).clamp(0.0, 100.0)))
        });
        self.last = Some(metrics);
        let memory = one_decimal(metrics.js_heap_bytes.max(0.0) / MIB);
        let mut changes = Vec::new();

        let above = cpu.is_some_and(|cpu| cpu > f64::from(limits.cpu_warn_percent));
        let since = if above {
            *self.busy_since.get_or_insert(now)
        } else {
            self.busy_since = None;
            now
        };
        let cpu_warn = above && now.duration_since(since) >= CPU_HOLD;
        if cpu_warn != self.cpu_warn {
            changes.push(match cpu {
                Some(cpu) if cpu_warn => Change::CpuWarn(cpu),
                _ => Change::CpuCalm,
            });
            self.cpu_warn = cpu_warn;
        }

        let ram_warn = memory > f64::from(limits.ram_warn_mb);
        if ram_warn != self.ram_warn {
            changes.push(if ram_warn {
                Change::RamWarn(memory)
            } else {
                Change::RamCalm
            });
            self.ram_warn = ram_warn;
        }

        let reading = ResourceReading {
            cpu_percent: cpu,
            memory_mb: Some(memory),
            cpu_warn,
            ram_warn,
            unavailable: None,
        };
        (reading, changes)
    }
}

#[cfg(test)]
#[path = "resources_tests.rs"]
mod tests;
