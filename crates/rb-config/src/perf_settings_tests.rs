use serde_json::json;

use crate::{PerformanceSettings, UserSettings};

#[test]
fn performance_defaults_throttle_hidden_views_and_warn_at_the_k3_budget() {
    let user: UserSettings = serde_json::from_value(json!({})).expect("settings");
    assert_eq!(user.performance, PerformanceSettings::default());
    assert!(user.performance.throttle_hidden);
    assert_eq!(user.performance.inactive_fps, 0);
    assert_eq!(user.performance.memory_warning_mb, 3072);
    assert!(
        user.performance.counters,
        "the timing counters record by default"
    );
    assert!(!user.extra.contains_key("performance"));
}

#[test]
fn performance_reads_its_section_in_camel_case() {
    let user: UserSettings = serde_json::from_value(json!({
        "performance": {
            "throttleHidden": false, "inactiveFps": 30, "memoryWarningMb": 0, "counters": false
        }
    }))
    .expect("settings");
    assert!(!user.performance.throttle_hidden);
    assert!(!user.performance.counters);
    assert_eq!(user.performance.inactive_fps, 30);
    assert_eq!(user.performance.memory_warning_mb, 0);
}

#[test]
fn performance_clamps_what_a_hand_edited_file_puts_out_of_range() {
    let settings = PerformanceSettings {
        throttle_hidden: true,
        inactive_fps: 500,
        memory_warning_mb: 10_000_000,
        counters: true,
    };
    let clamped = settings.clamped();
    assert_eq!(clamped.inactive_fps, crate::MAX_INACTIVE_FPS);
    assert_eq!(clamped.memory_warning_mb, crate::MAX_MEMORY_WARNING_MB);
    assert_eq!(
        PerformanceSettings::default().clamped(),
        PerformanceSettings::default()
    );
}
