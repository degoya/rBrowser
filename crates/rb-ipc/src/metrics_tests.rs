use serde_json::json;

use super::*;

#[test]
fn a_view_writes_only_what_it_has() {
    let metrics = ViewMetrics {
        view: ViewId(2),
        vitals: vec![
            VitalReading {
                metric: VitalMetric::Cls,
                value: Some(0.18),
                rating: Some(Rating::NeedsImprovement),
                unavailable: None,
                highlight: true,
            },
            VitalReading {
                metric: VitalMetric::Inp,
                value: None,
                rating: None,
                unavailable: Some(Unavailable::Pending),
                highlight: false,
            },
        ],
        soft: false,
        resources: Some(ResourceReading {
            cpu_percent: Some(12.5),
            memory_mb: Some(48.0),
            cpu_warn: false,
            ram_warn: true,
            unavailable: None,
        }),
        throttle: Some(ViewThrottle::CpuLimited { fps: 30 }),
    };
    assert_eq!(
        serde_json::to_value(&metrics).expect("serializes"),
        json!({
            "view": 2,
            "vitals": [
                { "metric": "cls", "value": 0.18, "rating": "needs-improvement", "highlight": true },
                { "metric": "inp", "unavailable": "pending" }
            ],
            "resources": { "cpuPercent": 12.5, "memoryMb": 48.0, "ramWarn": true },
            "throttle": { "state": "cpuLimited", "fps": 30 }
        })
    );
}

#[test]
fn an_empty_view_is_its_id() {
    let metrics = ViewMetrics {
        view: ViewId(1),
        vitals: Vec::new(),
        soft: false,
        resources: None,
        throttle: None,
    };
    assert_eq!(
        serde_json::to_value(&metrics).expect("serializes"),
        json!({ "view": 1 })
    );
    let hidden = serde_json::to_value(ViewThrottle::Hidden).expect("serializes");
    assert_eq!(hidden, json!({ "state": "hidden" }));
}
