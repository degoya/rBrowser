use std::collections::BTreeMap;

use super::*;
use crate::testing::FakeHost;

#[test]
fn defaults_without_settings() {
    let settings = Settings::read(&mut FakeHost::default());
    assert_eq!(settings, Settings::default());
    assert_eq!(settings.format, Format::Hex);
    assert_eq!(settings.loupe, 11);
    assert_eq!(settings.selector, "p, li");
    assert_eq!(
        (
            settings.optimal_min,
            settings.optimal_max,
            settings.warn_above
        ),
        (45, 75, 80)
    );
}

#[test]
fn the_persons_values_are_taken_when_they_hold() {
    let mut host = FakeHost {
        settings: BTreeMap::from([
            ("defaultFormat".into(), "\"oklch\"".into()),
            ("loupeSize".into(), "15".into()),
            ("textMeasure.selector".into(), "\"article p\"".into()),
            ("textMeasure.optimalMin".into(), "50".into()),
            ("textMeasure.optimalMax".into(), "70".into()),
            ("textMeasure.warnAbove".into(), "90".into()),
        ]),
        ..FakeHost::default()
    };
    let settings = Settings::read(&mut host);
    assert_eq!(settings.format, Format::Oklch);
    assert_eq!(settings.loupe, 15);
    assert_eq!(settings.selector, "article p");
    assert_eq!(
        (
            settings.optimal_min,
            settings.optimal_max,
            settings.warn_above
        ),
        (50, 70, 90)
    );
    assert_eq!(settings.to_json()["format"], "oklch");
    assert_eq!(settings.to_json()["textMeasure"]["warnAbove"], 90);
}

#[test]
fn values_that_do_not_hold_fall_back() {
    let mut host = FakeHost {
        settings: BTreeMap::from([
            ("loupeSize".into(), "12".into()),
            ("textMeasure.selector".into(), "\"\"".into()),
            ("textMeasure.optimalMin".into(), "90".into()),
            ("textMeasure.optimalMax".into(), "60".into()),
            ("textMeasure.warnAbove".into(), "\"many\"".into()),
        ]),
        ..FakeHost::default()
    };
    let settings = Settings::read(&mut host);
    assert_eq!(settings.loupe, 11);
    assert_eq!(settings.selector, "p, li");
    // A range whose minimum is above its maximum is the default range.
    assert_eq!((settings.optimal_min, settings.optimal_max), (45, 75));
    assert_eq!(settings.warn_above, 80);
}

#[test]
fn the_pipette_reads_the_window_image_unless_a_clip_is_chosen() {
    // RB-080-77: the window image is the way without flicker, the default.
    assert_eq!(
        Settings::read(&mut FakeHost::default()).capture,
        Capture::Window
    );
    for (value, capture) in [
        ("\"clip\"", Capture::Clip),
        ("\"window\"", Capture::Window),
        ("\"other\"", Capture::Window),
        ("3", Capture::Window),
    ] {
        let mut host = FakeHost {
            settings: BTreeMap::from([("capture".into(), value.into())]),
            ..FakeHost::default()
        };
        let settings = Settings::read(&mut host);
        assert_eq!(settings.capture, capture, "{value}");
        assert_eq!(settings.to_json()["capture"], capture.name());
    }
}
