use super::*;
use crate::load_test::gate::Standing;
use crate::load_test::scenario::Profile;
use crate::load_test::stats::{Failed, Hit};

fn meta() -> Meta {
    Meta {
        scenario: "s.json".into(),
        name: "S".into(),
        started_at: 1,
        profile: Profile {
            users: 50,
            ramp_up_s: 10,
            hold_s: 40,
            ramp_down_s: 10,
            rps: None,
        },
        rps_limit: 200.0,
        environment: None,
        targets: vec![("localhost".into(), Standing::Local)],
        steps: vec!["a.http".into()],
    }
}

#[test]
fn a_running_snapshot_carries_the_live_numbers() {
    let mut recorder = Recorder::new(1, 10);
    recorder.record(&Hit {
        step: 0,
        ms: 12.0,
        status: Some(503),
        failed: Some(Failed::Http5xx),
    });
    recorder.tick(1, 5);
    let tick = Tick {
        t: 1,
        elapsed_s: 1.0,
        users: 5,
        target: 6,
    };
    let live = snapshot(&meta(), &recorder, &tick, None, None);
    assert_eq!(live["running"], true);
    assert_eq!(live["durationS"], 60);
    assert_eq!(live["users"], 5);
    assert_eq!(live["targetUsers"], 6);
    assert_eq!(live["totals"]["requests"], 1);
    assert_eq!(live["errors"]["http5xx"], 1);
    assert_eq!(live["series"][0]["p50"], 12.0);
    assert_eq!(live["targets"][0], "localhost");
    assert_eq!(live["end"], Value::Null);
}

#[test]
fn the_last_snapshot_names_the_end_and_the_report_and_the_series_is_cut() {
    let mut recorder = Recorder::new(1, 10);
    for t in 1..=400 {
        recorder.tick(t, 1);
    }
    let tick = Tick {
        t: 400,
        elapsed_s: 400.0,
        users: 0,
        target: 0,
    };
    let last = snapshot(&meta(), &recorder, &tick, Some(End::Stopped), Some("1-s"));
    assert_eq!(last["running"], false);
    assert_eq!(last["end"], "stopped");
    assert_eq!(last["reportId"], "1-s");
    let series = last["series"].as_array().expect("series");
    assert_eq!(series.len(), SERIES_SHOWN);
    assert_eq!(series[0]["t"], 101);
}
