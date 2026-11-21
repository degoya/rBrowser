use rb_engine::{Emulation, FieldSupport};
use serde_json::json;

use super::*;

#[test]
fn the_probe_reads_the_chrome_and_the_viewport() {
    let probe = parse_probe(&json!([0, 74, 375, 667])).expect("four numbers");
    assert_eq!(
        probe,
        Probe {
            chrome: (0, 74),
            inner: (375, 667),
        }
    );
    assert_eq!(parse_probe(&json!([1, 2, 3])), None);
    assert_eq!(parse_probe(&json!("375")), None);
    assert_eq!(parse_probe(&json!([0, 0, -5, 10])), None);
}

#[test]
fn the_window_is_the_viewport_and_its_chrome() {
    let probe = Probe {
        chrome: (0, 74),
        inner: (800, 600),
    };
    assert_eq!(window_size((375, 667), &probe), (375, 741));
    // A probe that makes no sense never asks for a window of no size.
    let odd = Probe {
        chrome: (-900, -900),
        inner: (800, 600),
    };
    assert_eq!(window_size((375, 667), &odd), (1, 1));
}

#[test]
fn the_window_rect_bodies_carry_only_what_is_set() {
    assert_eq!(
        size_body((375, 741)),
        json!({ "width": 375, "height": 741 })
    );
    let rect = rb_engine::external::WindowRect {
        x: 10,
        y: -20,
        width: 1,
        height: 1,
    };
    assert_eq!(position_body(rect), json!({ "x": 10, "y": -20 }));
}

#[test]
fn a_viewport_reached_is_applied() {
    let emulation = Emulation {
        width: 375,
        height: 667,
        ..Emulation::default()
    };
    let report = report(&emulation, Some((375, 667)));
    assert_eq!(report.fields[0].field, "viewport");
    assert_eq!(report.fields[0].support, FieldSupport::Applied);
}

#[test]
fn a_viewport_safari_would_not_take_is_approximated_with_what_it_shows() {
    let emulation = Emulation {
        width: 320,
        height: 568,
        ..Emulation::default()
    };
    let report = report(&emulation, Some((500, 568)));
    let FieldSupport::Approximated(reason) = &report.fields[0].support else {
        panic!("approximated");
    };
    assert!(reason.contains("500x568"), "{reason}");
    assert!(report.unsupported().iter().any(|field| field == "viewport"));
    // Unknown is not wrong: nothing read back keeps the table's answer.
    let unknown = super::report(&emulation, None);
    assert_eq!(unknown.fields[0].support, FieldSupport::Applied);
}

#[test]
fn ios_reports_the_device_and_the_size_it_shows() {
    let emulation = Emulation {
        width: 390,
        height: 844,
        ..Emulation::default()
    };
    let same = ios_report(&emulation, "iPhone 16 (iOS 18.2)", Some((390, 844)));
    assert_eq!(
        same,
        EmulationReport::for_engine(EngineKind::Ios, &emulation),
        "the published table while the sizes agree"
    );
    let other = ios_report(&emulation, "iPhone 16 (iOS 18.2)", Some((393, 852)));
    assert_eq!(
        other.fields[0].support,
        FieldSupport::Unsupported("iPhone 16 (iOS 18.2) shows 393x852".to_owned())
    );
    assert_eq!(
        ios_report(&emulation, "iPhone (USB)", None),
        EmulationReport::for_engine(EngineKind::Ios, &emulation)
    );
}
