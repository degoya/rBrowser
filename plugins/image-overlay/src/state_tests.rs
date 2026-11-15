use super::*;
use serde_json::json;

#[test]
fn the_state_reads_and_writes_in_camel_case_with_lower_case_names() {
    let state = State::default();
    let value = serde_json::to_value(&state).expect("json");
    assert_eq!(
        value,
        json!({
            "enabled": false, "opacity": 50, "blend": "normal", "invert": false,
            "anchor": "document", "offset": { "x": 0.0, "y": 0.0 },
            "split": { "on": false, "position": 50.0 }, "autoScaleDpr": true, "designs": {}
        })
    );
    let blend: Blend = serde_json::from_value(json!("difference")).expect("blend");
    assert_eq!(blend, Blend::Difference);
    assert!(serde_json::from_value::<Blend>(json!("darken")).is_err());
}

#[test]
fn the_overrides_win_over_the_shared_layer_and_null_drops_a_design() {
    let shared = Shared {
        designs: BTreeMap::from([
            ("mobile".to_owned(), "designs/mobile.png".to_owned()),
            ("desktop".to_owned(), "designs/desktop.png".to_owned()),
        ]),
        ..Shared::default()
    };
    let overrides = Overrides {
        enabled: Some(true),
        opacity: Some(30),
        blend: Some(Blend::Difference),
        split: Some(Split {
            on: true,
            position: 30.0,
        }),
        designs: BTreeMap::from([
            ("mobile".to_owned(), None),
            ("tablet".to_owned(), Some("designs/tablet.png".to_owned())),
        ]),
        ..Overrides::default()
    };
    let state = effective(&shared, &overrides);
    assert!(state.enabled);
    assert_eq!(state.opacity, 30);
    assert_eq!(state.blend, Blend::Difference);
    assert_eq!(state.anchor, Anchor::Document);
    assert_eq!(state.split.position, 30.0);
    assert_eq!(
        state.designs,
        BTreeMap::from([
            ("desktop".to_owned(), "designs/desktop.png".to_owned()),
            ("tablet".to_owned(), "designs/tablet.png".to_owned()),
        ])
    );
}

#[test]
fn values_out_of_range_are_named() {
    assert_eq!(
        Overrides {
            opacity: Some(101),
            ..Overrides::default()
        }
        .check(),
        Err("opacity")
    );
    assert_eq!(
        Overrides {
            offset: Some(Offset {
                x: f64::NAN,
                y: 0.0
            }),
            ..Overrides::default()
        }
        .check(),
        Err("offset")
    );
    assert_eq!(
        Overrides {
            offset: Some(Offset {
                x: 0.0,
                y: MAX_OFFSET + 1.0
            }),
            ..Overrides::default()
        }
        .check(),
        Err("offset")
    );
    assert_eq!(
        Overrides {
            split: Some(Split {
                on: true,
                position: 100.5
            }),
            ..Overrides::default()
        }
        .check(),
        Err("split")
    );
    assert_eq!(
        Overrides {
            designs: BTreeMap::from([("a b".to_owned(), None)]),
            ..Overrides::default()
        }
        .check(),
        Err("designs")
    );
    assert_eq!(Overrides::default().check(), Ok(()));
}

#[test]
fn a_design_override_outside_the_designs_folder_is_refused() {
    let overrides = Overrides {
        designs: BTreeMap::from([("mobile".to_owned(), Some("../x.png".to_owned()))]),
        ..Overrides::default()
    };
    assert_eq!(overrides.check(), Err("designs"));
}

#[test]
fn whether_the_views_must_load_their_image_again() {
    let before = State::default();
    let mut after = before.clone();
    after.opacity = 20;
    assert!(!reload(&before, &after));
    after.enabled = true;
    assert!(reload(&before, &after));
    let mut moved = before.clone();
    moved
        .designs
        .insert("mobile".to_owned(), "designs/m.png".to_owned());
    assert!(reload(&before, &moved));
    let mut scale = before.clone();
    scale.auto_scale_dpr = false;
    assert!(reload(&before, &scale));
}
