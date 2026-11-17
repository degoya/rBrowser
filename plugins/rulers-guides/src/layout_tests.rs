use super::*;
use crate::model::{Axis, ColumnSpec, Guide, GuideUnit};
use rb_plugin_api::test_support::MemoryHost;
use rb_plugin_api::{Capability, ViewInfo};
use std::collections::BTreeMap;

fn view(id: u32, breakpoint: &str) -> ViewInfo {
    ViewInfo {
        id,
        label: breakpoint.to_owned(),
        breakpoint: breakpoint.to_owned(),
        engine: "chromium".to_owned(),
        width: 375,
        height: 800,
        dpr: 3.0,
        url: "https://example.test/".to_owned(),
        title: String::new(),
    }
}

fn geometry(width: f64) -> Geometry {
    Geometry {
        client_width: width,
        inner_width: width,
    }
}

#[test]
fn the_breakpoint_comes_from_the_views() {
    let mut host = MemoryHost::new().grant(Capability::Views);
    host.views = vec![view(1, "mobile"), view(2, "desktop"), view(3, "mobile")];
    assert_eq!(breakpoint_of(&mut host, 2).as_deref(), Some("desktop"));
    assert_eq!(breakpoint_of(&mut host, 9), None);
    assert_eq!(breakpoints(&mut host), vec!["desktop", "mobile"]);
    // Not served: no breakpoint, no list.
    assert_eq!(breakpoint_of(&mut MemoryHost::new(), 1), None);
    assert!(breakpoints(&mut MemoryHost::new()).is_empty());
}

#[test]
fn each_view_draws_its_breakpoints_columns_and_the_same_guides() {
    let state = State {
        guides: vec![
            Guide {
                id: 1,
                axis: Axis::X,
                unit: GuideUnit::Percent,
                value: 50.0,
            },
            Guide {
                id: 2,
                axis: Axis::X,
                unit: GuideUnit::Px,
                value: 320.0,
            },
        ],
        columns: BTreeMap::from([
            (
                "mobile".to_owned(),
                ColumnSpec {
                    columns: Some(4),
                    gutter: Some(16.0),
                    margin: Some(16.0),
                    ..ColumnSpec::default()
                },
            ),
            (
                "desktop".to_owned(),
                ColumnSpec {
                    columns: Some(12),
                    gutter: Some(24.0),
                    margin: Some(12.0),
                    max_width: Some(1200.0),
                    ..ColumnSpec::default()
                },
            ),
        ]),
        ..State::default()
    };
    let mobile = layout(&state, Some("mobile".to_owned()), geometry(375.0));
    let desktop = layout(&state, Some("desktop".to_owned()), geometry(1440.0));
    assert_eq!(mobile.columns.as_ref().map(|c| c.columns.len()), Some(4));
    assert_eq!(desktop.columns.as_ref().map(|c| c.columns.len()), Some(12));
    assert_eq!(mobile.guides[0].pos, 187.5);
    assert_eq!(desktop.guides[0].pos, 720.0);
    assert_eq!(mobile.guides[1].pos, 320.0);
    assert_eq!(desktop.guides[1].pos, 320.0);
    // A breakpoint without a setup and no `*`: no columns.
    assert!(
        layout(&state, Some("tablet".to_owned()), geometry(800.0))
            .columns
            .is_none()
    );
}

#[test]
fn a_geometry_is_held_to_finite_lengths() {
    assert!(geometry(375.0).check().is_ok());
    assert_eq!(geometry(f64::INFINITY).check(), Err("geometry"));
    assert_eq!(geometry(-1.0).check(), Err("geometry"));
}
