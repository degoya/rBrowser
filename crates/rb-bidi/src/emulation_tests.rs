use std::collections::BTreeMap;

use rb_config::{ColorScheme, NetworkConditions, Orientation};
use rb_engine::{EmulationReport, EngineKind, FieldSupport};
use serde_json::json;

use super::*;

fn phone() -> Emulation {
    Emulation {
        width: 393,
        height: 852,
        dpr: 3.0,
        mobile: true,
        touch: true,
        user_agent: Some("Mozilla/5.0 (iPhone)".to_owned()),
        locale: Some("fr-FR".to_owned()),
        color_scheme: Some(ColorScheme::Dark),
        reduced_motion: Some(true),
        orientation: Some(Orientation::Landscape),
        network: Some(NetworkConditions {
            offline: true,
            ..NetworkConditions::default()
        }),
        headers: BTreeMap::from([("X-Env".to_owned(), "dev".to_owned())]),
        ..Emulation::default()
    }
}

fn find<'a>(overrides: &'a [Override], method: &str) -> &'a Override {
    overrides
        .iter()
        .find(|o| o.command.method == method)
        .unwrap_or_else(|| panic!("no {method}"))
}

#[test]
fn the_viewport_is_size_and_dpr() {
    assert_eq!(
        viewport("c1", &phone()).params,
        json!({ "context": "c1", "viewport": { "width": 393, "height": 852 }, "devicePixelRatio": 3.0 })
    );
}

/// A breakpoint without a DPR (0: the display's) sends no `devicePixelRatio` - Firefox 160
/// refuses 0 and the view would not start (RB-080-38).
#[test]
fn a_dpr_of_zero_is_left_to_the_display() {
    let display = Emulation {
        dpr: 0.0,
        ..phone()
    };
    let params = viewport("c1", &display).params;
    assert_eq!(params["viewport"], json!({ "width": 393, "height": 852 }));
    assert!(params.get("devicePixelRatio").is_none(), "{params}");
}

/// In its grid cell (RB-080-40) the view keeps the breakpoint's width and DPR and takes the
/// cell's height; a breakpoint without a DPR gets 1, so a screenshot stays breakpoint-sized.
#[test]
fn in_a_cell_the_viewport_takes_the_cells_height_and_keeps_the_breakpoints_dpr() {
    let cell = Some(CellScale {
        factor: 0.82,
        width: 393,
        height: 610,
    });
    assert_eq!(
        viewport_in("c1", &phone(), cell).params,
        json!({ "context": "c1", "viewport": { "width": 393, "height": 610 }, "devicePixelRatio": 3.0 })
    );
    let display = Emulation {
        dpr: 0.0,
        ..phone()
    };
    assert_eq!(wanted_viewport(&display, cell), (393, 610, 1.0));
    assert_eq!(wanted_viewport(&display, None), (393, 852, 0.0));
    assert_eq!(
        viewport_in("c1", &phone(), None).params,
        viewport("c1", &phone()).params
    );
}

#[test]
fn every_override_is_set_for_the_view_alone() {
    let overrides = overrides("c1", &phone());
    for item in &overrides {
        assert_eq!(
            item.command.params["contexts"],
            json!(["c1"]),
            "{:?}",
            item.command
        );
    }
    let params = |method| find(&overrides, method).command.params.clone();
    assert_eq!(
        params("emulation.setUserAgentOverride")["userAgent"],
        "Mozilla/5.0 (iPhone)"
    );
    assert_eq!(params("emulation.setLocaleOverride")["locale"], "fr-FR");
    assert_eq!(
        params("emulation.setMediaFeaturesOverride")["features"],
        json!({ "prefers-color-scheme": "dark" })
    );
    assert_eq!(
        params("emulation.setScreenOrientationOverride")["screenOrientation"],
        json!({ "natural": "portrait", "type": "landscape-primary" })
    );
    assert_eq!(
        params("emulation.setScreenSettingsOverride")["screenArea"],
        json!({ "width": 393, "height": 852 })
    );
    assert_eq!(
        params("emulation.setNetworkConditions")["networkConditions"],
        json!({ "type": "offline" })
    );
    assert_eq!(
        params("network.setExtraHeaders")["headers"],
        json!([{ "name": "X-Env", "value": { "type": "string", "value": "dev" } }])
    );
    assert_eq!(
        find(&overrides, "emulation.setUserAgentOverride").field,
        Some("userAgent")
    );
    assert_eq!(
        find(&overrides, "emulation.setScreenSettingsOverride").field,
        None
    );
}

#[test]
fn fields_not_asked_for_are_reset() {
    let overrides = overrides("c1", &Emulation::default());
    let params = |method| find(&overrides, method).command.params.clone();
    assert_eq!(
        params("emulation.setUserAgentOverride")["userAgent"],
        json!(null)
    );
    assert_eq!(params("emulation.setLocaleOverride")["locale"], json!(null));
    assert_eq!(
        params("emulation.setMediaFeaturesOverride")["features"],
        json!(null)
    );
    assert_eq!(
        params("emulation.setScreenOrientationOverride")["screenOrientation"],
        json!(null)
    );
    assert_eq!(
        params("emulation.setNetworkConditions")["networkConditions"],
        json!(null)
    );
    assert_eq!(params("network.setExtraHeaders")["headers"], json!([]));
}

#[test]
fn the_report_is_the_published_table_unless_firefox_lacks_a_command() {
    let emulation = phone();
    assert_eq!(
        report(&emulation, &[]),
        EmulationReport::for_engine(EngineKind::Firefox, &emulation)
    );
    let report = report(&emulation, &["userAgent"]);
    let user_agent = report
        .fields
        .iter()
        .find(|field| field.field == "userAgent")
        .expect("userAgent");
    assert!(matches!(user_agent.support, FieldSupport::Unsupported(_)));
    let marked = report.unsupported();
    for field in ["touch", "reducedMotion", "userAgent", "mobile"] {
        assert!(marked.contains(&field.to_owned()), "{field} in {marked:?}");
    }
    assert!(!marked.contains(&"locale".to_owned()));
}
