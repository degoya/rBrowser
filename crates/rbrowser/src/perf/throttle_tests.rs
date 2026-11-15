use super::*;

const ON: ThrottleSettings = ThrottleSettings {
    throttle_hidden: true,
    inactive_fps: 0,
};

fn facts(view: u32, zero_area: bool, active: bool) -> ViewFacts {
    ViewFacts {
        view: ViewId(view),
        zero_area,
        active,
        attached: true,
        emulation_rate: 1.0,
    }
}

#[test]
fn a_view_without_area_is_hidden_once_and_shown_with_a_catch_up() {
    let mut throttle = Throttle::default();
    assert_eq!(throttle.plan(ON, &[facts(1, false, true)]), vec![]);
    assert_eq!(
        throttle.plan(ON, &[facts(1, true, true)]),
        vec![ThrottleAction::Hide(ViewId(1))]
    );
    assert_eq!(throttle.state(ViewId(1)), ThrottleState::Hidden);
    // Nothing changed: nothing to do.
    assert_eq!(throttle.plan(ON, &[facts(1, true, true)]), vec![]);
    assert_eq!(
        throttle.plan(ON, &[facts(1, false, true)]),
        vec![
            ThrottleAction::Show(ViewId(1)),
            ThrottleAction::CatchUp(ViewId(1))
        ]
    );
    assert_eq!(throttle.state(ViewId(1)), ThrottleState::Visible);
}

#[test]
fn with_throttling_off_a_view_without_area_keeps_rendering() {
    let off = ThrottleSettings {
        throttle_hidden: false,
        ..ON
    };
    let mut throttle = Throttle::default();
    assert_eq!(throttle.plan(off, &[facts(1, true, false)]), vec![]);
    assert_eq!(throttle.state(ViewId(1)), ThrottleState::Visible);
}

#[test]
fn switching_throttling_off_shows_the_hidden_views_and_on_hides_them_again() {
    let off = ThrottleSettings {
        throttle_hidden: false,
        ..ON
    };
    let mut throttle = Throttle::default();
    throttle.plan(ON, &[facts(1, true, false), facts(2, false, true)]);
    assert_eq!(
        throttle.plan(off, &[facts(1, true, false), facts(2, false, true)]),
        vec![
            ThrottleAction::Show(ViewId(1)),
            ThrottleAction::CatchUp(ViewId(1))
        ]
    );
    assert_eq!(
        throttle.plan(ON, &[facts(1, true, false), facts(2, false, true)]),
        vec![ThrottleAction::Hide(ViewId(1))]
    );
}

#[test]
fn inactive_visible_views_get_a_cpu_budget_and_lose_it_when_active() {
    let thirty = ThrottleSettings {
        inactive_fps: 30,
        ..ON
    };
    let mut throttle = Throttle::default();
    assert_eq!(
        throttle.plan(thirty, &[facts(1, false, true), facts(2, false, false)]),
        vec![ThrottleAction::CpuRate {
            view: ViewId(2),
            rate: 2.0
        }]
    );
    assert_eq!(
        throttle.state(ViewId(2)),
        ThrottleState::CpuLimited { fps: 30 }
    );
    assert_eq!(throttle.state(ViewId(1)), ThrottleState::Visible);
    // View 2 becomes active: full speed for it, the budget for view 1.
    assert_eq!(
        throttle.plan(thirty, &[facts(1, false, false), facts(2, false, true)]),
        vec![
            ThrottleAction::CpuRate {
                view: ViewId(1),
                rate: 2.0
            },
            ThrottleAction::CpuRate {
                view: ViewId(2),
                rate: 1.0
            }
        ]
    );
}

#[test]
fn a_hidden_view_gets_no_cpu_budget_so_sync_reaches_it_at_full_speed() {
    let thirty = ThrottleSettings {
        inactive_fps: 30,
        ..ON
    };
    let mut throttle = Throttle::default();
    assert_eq!(
        throttle.plan(thirty, &[facts(2, true, false)]),
        vec![ThrottleAction::Hide(ViewId(2))]
    );
}

#[test]
fn the_cpu_budget_never_lowers_the_views_own_cpu_emulation() {
    let thirty = ThrottleSettings {
        inactive_fps: 30,
        ..ON
    };
    let mut throttle = Throttle::default();
    let mut slow = facts(2, false, false);
    slow.emulation_rate = 4.0;
    // The emulation already sent 4 at the attach.
    throttle.session_reset(ViewId(2), 4.0);
    assert_eq!(throttle.plan(thirty, &[slow]), vec![]);
    // Active again: back to the emulation's rate, not to 1 - and it already is.
    slow.active = true;
    assert_eq!(throttle.plan(thirty, &[slow]), vec![]);
}

#[test]
fn a_view_not_attached_gets_no_cdp_call_until_its_session_exists() {
    let thirty = ThrottleSettings {
        inactive_fps: 30,
        ..ON
    };
    let mut throttle = Throttle::default();
    let mut fresh = facts(2, false, false);
    fresh.attached = false;
    assert_eq!(throttle.plan(thirty, &[fresh]), vec![]);
    fresh.attached = true;
    throttle.session_reset(ViewId(2), 1.0);
    assert_eq!(
        throttle.plan(thirty, &[fresh]),
        vec![ThrottleAction::CpuRate {
            view: ViewId(2),
            rate: 2.0
        }]
    );
}

#[test]
fn a_closed_view_is_forgotten() {
    let mut throttle = Throttle::default();
    throttle.plan(ON, &[facts(1, true, false)]);
    throttle.plan(ON, &[]);
    assert_eq!(throttle.state(ViewId(1)), ThrottleState::Visible);
    // Reopened under the same id (never happens, ids are not reused): a fresh start.
    assert_eq!(
        throttle.plan(ON, &[facts(1, true, false)]),
        vec![ThrottleAction::Hide(ViewId(1))]
    );
}

#[test]
fn the_cpu_rate_follows_the_frame_budget() {
    assert!((cpu_rate(0) - 1.0).abs() < f64::EPSILON);
    assert!((cpu_rate(60) - 1.0).abs() < f64::EPSILON);
    assert!((cpu_rate(90) - 1.0).abs() < f64::EPSILON);
    assert!((cpu_rate(30) - 2.0).abs() < f64::EPSILON);
    assert!((cpu_rate(20) - 3.0).abs() < f64::EPSILON);
    assert!((cpu_rate(1) - MAX_CPU_RATE).abs() < f64::EPSILON);
}

#[test]
fn the_state_serialises_for_the_vitals_label() {
    let limited = serde_json::to_value(ThrottleState::CpuLimited { fps: 30 }).expect("json");
    assert_eq!(
        limited,
        serde_json::json!({ "state": "cpuLimited", "fps": 30 })
    );
    let hidden = serde_json::to_value(ThrottleState::Hidden).expect("json");
    assert_eq!(hidden, serde_json::json!({ "state": "hidden" }));
}
