use super::{ALL, Group, Overlay, Simulation, Step, spec};

const IDENTITY: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

fn matrix_of(simulation: Simulation, severity: f64) -> [[f64; 3]; 3] {
    let spec = spec(simulation, severity, 2.0).expect("a spec");
    let Some(Step::Matrix { values }) = spec.filter.first() else {
        panic!("{simulation:?} starts with a matrix");
    };
    // The alpha row and the offsets stay untouched.
    assert_eq!(&values[15..20], &[0.0, 0.0, 0.0, 1.0, 0.0]);
    for row in 0..3 {
        assert_eq!(values[row * 5 + 3], 0.0);
        assert_eq!(values[row * 5 + 4], 0.0);
    }
    [
        [values[0], values[1], values[2]],
        [values[5], values[6], values[7]],
        [values[10], values[11], values[12]],
    ]
}

#[test]
fn there_are_fifteen_simulations_in_three_groups() {
    assert_eq!(ALL.len(), 15);
    let count = |group| {
        ALL.iter()
            .filter(|kind| kind.group() == Some(group))
            .count()
    };
    assert_eq!(count(Group::Color), 8);
    assert_eq!(count(Group::Vision), 5);
    assert_eq!(count(Group::Environment), 2);
    assert_eq!(Simulation::None.group(), None);
    let mut ids: Vec<&str> = ALL.iter().map(|kind| kind.id()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 15);
}

#[test]
fn ids_read_and_write_as_the_mcp_tool_names_them() {
    for kind in ALL.iter().copied().chain([Simulation::None]) {
        let text = serde_json::to_string(&kind).expect("serialises");
        assert_eq!(text, format!("\"{}\"", kind.id()));
        let back: Simulation = serde_json::from_str(&text).expect("reads");
        assert_eq!(back, kind);
    }
    assert_eq!(Simulation::MacularDegeneration.id(), "macular-degeneration");
    assert!(serde_json::from_str::<Simulation>("\"Protanopia\"").is_err());
}

#[test]
fn chromium_renders_six_types_itself() {
    let native: Vec<(&str, &str)> = ALL
        .iter()
        .filter_map(|kind| kind.native().map(|cdp| (kind.id(), cdp)))
        .collect();
    assert_eq!(
        native,
        [
            ("protanopia", "protanopia"),
            ("deuteranopia", "deuteranopia"),
            ("tritanopia", "tritanopia"),
            ("achromatopsia", "achromatopsia"),
            ("myopia", "blurredVision"),
            ("low-contrast", "reducedContrast"),
        ]
    );
    assert_eq!(Simulation::Tritanomaly.native(), None);
    assert_eq!(Simulation::None.native(), None);
}

#[test]
fn the_dichromacies_are_machados_matrices_at_full_severity() {
    // Machado, Oliveira and Fernandes (2009), severity 1.0 - the matrices Chromium's own
    // emulation uses, rounded to three places there.
    let protanopia = matrix_of(Simulation::Protanopia, 0.3);
    assert_eq!(protanopia[0], [0.1523, 1.0526, -0.2049]);
    let deuteranopia = matrix_of(Simulation::Deuteranopia, 1.0);
    assert_eq!(deuteranopia[1], [0.2801, 0.6725, 0.0474]);
    let tritanopia = matrix_of(Simulation::Tritanopia, 1.0);
    assert_eq!(tritanopia[2], [0.0047, 0.6914, 0.3039]);
    // Every row keeps white white.
    for matrix in [protanopia, deuteranopia, tritanopia] {
        for row in matrix {
            assert!((row.iter().sum::<f64>() - 1.0).abs() < 0.002, "{row:?}");
        }
    }
}

#[test]
fn an_anomaly_lies_between_normal_vision_and_its_dichromacy() {
    for (anomaly, full) in [
        (Simulation::Protanomaly, Simulation::Protanopia),
        (Simulation::Deuteranomaly, Simulation::Deuteranopia),
        (Simulation::Tritanomaly, Simulation::Tritanopia),
        (Simulation::Achromatomaly, Simulation::Achromatopsia),
    ] {
        assert_eq!(matrix_of(anomaly, 0.0), IDENTITY, "{anomaly:?}");
        assert_eq!(matrix_of(anomaly, 1.0), matrix_of(full, 1.0), "{anomaly:?}");
        let half = matrix_of(anomaly, 0.5);
        let target = matrix_of(full, 1.0);
        for row in 0..3 {
            for column in 0..3 {
                let expected = (IDENTITY[row][column] + target[row][column]) / 2.0;
                assert!((half[row][column] - expected).abs() < 0.0002);
            }
        }
        assert_eq!(anomaly.default_severity(), Some(0.6));
        assert_eq!(full.default_severity(), None);
    }
}

#[test]
fn achromatopsia_keeps_the_luminance_alone() {
    let matrix = matrix_of(Simulation::Achromatopsia, 1.0);
    for row in matrix {
        assert_eq!(row, [0.213, 0.715, 0.072]);
    }
}

#[test]
fn the_impairments_and_surroundings_follow_their_strength() {
    let myopia = spec(Simulation::Myopia, 0.5, 4.0).expect("spec");
    assert_eq!(myopia.filter, [Step::Blur { px: 2.0 }]);
    assert_eq!(myopia.overlay, None);

    let contrast = spec(Simulation::LowContrast, 1.0, 2.0).expect("spec");
    assert_eq!(contrast.filter, [Step::Contrast { amount: 0.5 }]);

    let dimmed = spec(Simulation::Dimmed, 0.5, 2.0).expect("spec");
    assert_eq!(dimmed.filter, [Step::Brightness { amount: 0.7 }]);

    let sunlight = spec(Simulation::Sunlight, 1.0, 2.0).expect("spec");
    assert_eq!(
        sunlight.filter,
        [
            Step::Brightness { amount: 1.4 },
            Step::Contrast { amount: 0.5 },
            Step::Saturate { amount: 0.5 },
        ]
    );

    let cataract = spec(Simulation::Cataract, 1.0, 2.0).expect("spec");
    assert!(matches!(cataract.filter.last(), Some(Step::Matrix { .. })));
    assert!(cataract.filter.contains(&Step::Blur { px: 2.0 }));
    assert!(cataract.filter.contains(&Step::Contrast { amount: 0.6 }));
    assert_eq!(cataract.overlay, Some(Overlay::Glare { opacity: 0.35 }));

    let glaucoma = spec(Simulation::Glaucoma, 1.0, 2.0).expect("spec");
    assert!(glaucoma.filter.is_empty());
    assert_eq!(
        glaucoma.overlay,
        Some(Overlay::Tunnel {
            clear: 0.1,
            fade: 0.25,
            opacity: 0.95
        })
    );

    let macular = spec(Simulation::MacularDegeneration, 0.0, 2.0).expect("spec");
    assert_eq!(
        macular.overlay,
        Some(Overlay::Spot {
            size: 0.1,
            blur_px: 6.0,
            opacity: 0.85
        })
    );
}

#[test]
fn a_stronger_severity_is_never_milder() {
    let tunnel =
        |severity| match spec(Simulation::Glaucoma, severity, 2.0).and_then(|spec| spec.overlay) {
            Some(Overlay::Tunnel { clear, .. }) => clear,
            other => panic!("{other:?}"),
        };
    assert!(tunnel(0.9) < tunnel(0.2));
    let dim = |severity| match spec(Simulation::Dimmed, severity, 2.0)
        .map(|spec| spec.filter)
        .as_deref()
    {
        Some([Step::Brightness { amount }]) => *amount,
        other => panic!("{other:?}"),
    };
    assert!(dim(0.9) < dim(0.2));
}

#[test]
fn none_has_no_spec_and_out_of_range_strengths_are_clamped() {
    assert_eq!(spec(Simulation::None, 1.0, 2.0), None);
    assert_eq!(
        spec(Simulation::Dimmed, 7.0, 2.0),
        spec(Simulation::Dimmed, 1.0, 2.0)
    );
    assert_eq!(
        spec(Simulation::Myopia, f64::NAN, 2.0),
        spec(Simulation::Myopia, 0.0, 2.0)
    );
}

#[test]
fn a_spec_reads_as_the_page_script_expects() {
    let spec = spec(Simulation::Cataract, 1.0, 2.0).expect("spec");
    let json = serde_json::to_value(&spec).expect("serialises");
    assert_eq!(
        json["filter"][0],
        serde_json::json!({ "op": "blur", "px": 2.0 })
    );
    assert_eq!(json["filter"][2]["op"], "matrix");
    assert_eq!(
        json["filter"][2]["values"].as_array().map(Vec::len),
        Some(20)
    );
    assert_eq!(
        json["overlay"],
        serde_json::json!({ "kind": "glare", "opacity": 0.35 })
    );
    let spot = super::spec(Simulation::MacularDegeneration, 1.0, 2.0).expect("spec");
    let json = serde_json::to_value(&spot).expect("serialises");
    assert_eq!(
        json["overlay"],
        serde_json::json!({ "kind": "spot", "size": 0.35, "blurPx": 16.0, "opacity": 0.85 })
    );
}

/// The drawings the e2e suite (`web/e2e/`) checks in a real Chromium: every simulation at its
/// default strength with the default blur. `RB_UPDATE_FIXTURES=1 cargo test` writes the file.
#[test]
fn the_e2e_suite_draws_the_catalogues_specs() {
    let specs: std::collections::BTreeMap<&str, super::Spec> = ALL
        .iter()
        .map(|kind| {
            let severity = kind.default_severity().unwrap_or(1.0);
            let drawn = spec(*kind, severity, crate::state::DEFAULT_BLUR_PX).expect("a spec");
            (kind.id(), drawn)
        })
        .collect();
    let text = serde_json::to_string_pretty(&specs).expect("serialises") + "\n";
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/specs.json");
    if std::env::var_os("RB_UPDATE_FIXTURES").is_some() {
        std::fs::write(path, &text).expect("written");
    }
    let on_disk = std::fs::read_to_string(path).unwrap_or_default();
    assert!(
        on_disk == text,
        "tests/fixtures/specs.json is stale: RB_UPDATE_FIXTURES=1 cargo test"
    );
}
