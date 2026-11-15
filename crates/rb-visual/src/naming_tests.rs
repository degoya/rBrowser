use std::path::PathBuf;

use super::*;

fn vars() -> PathVars {
    PathVars {
        stamp: Stamp::from_unix(1_794_312_187),
        env: Some("staging".to_owned()),
        breakpoint: "mobile".to_owned(),
        view: 3,
        mode: "viewport".to_owned(),
        width: 393,
        height: 852,
        dpr: 3.0,
    }
}

#[test]
fn stamp_is_the_utc_calendar_date() {
    // 2026-11-10 12:03:07 UTC.
    let stamp = Stamp::from_unix(1_794_312_187);
    assert_eq!(stamp.day(), "2026-11-10");
    assert_eq!(stamp.time(), "12-03-07");
    assert_eq!(stamp.date_time(), "2026-11-10_12-03-07");
    assert_eq!(Stamp::from_unix(0).date_time(), "1970-01-01_00-00-00");
    // A leap day.
    assert_eq!(Stamp::from_unix(951_782_400).day(), "2000-02-29");
}

#[test]
fn default_pattern_is_a_folder_per_run_and_a_file_per_breakpoint() {
    let path = expand(DEFAULT_PATTERN, &vars()).expect("valid pattern");
    assert_eq!(path, PathBuf::from("2026-11-10_12-03-07").join("mobile"));
}

#[test]
fn every_placeholder_expands() {
    let path = expand(
        "{date}/{env}/{day}-{time}/{breakpoint}_{view}_{mode}_{width}x{height}@{dpr}",
        &vars(),
    )
    .expect("valid pattern");
    assert_eq!(
        path,
        PathBuf::from("2026-11-10_12-03-07")
            .join("staging")
            .join("2026-11-10-12-03-07")
            .join("mobile_3_viewport_393x852@3x")
    );
}

#[test]
fn no_environment_is_default() {
    let mut vars = vars();
    vars.env = None;
    assert_eq!(
        expand("{env}/{breakpoint}", &vars).expect("valid"),
        PathBuf::from("default").join("mobile")
    );
}

#[test]
fn unknown_placeholders_and_broken_braces_are_refused() {
    assert!(matches!(
        expand("{date}/{nope}", &vars()),
        Err(VisualError::Pattern(_))
    ));
    assert!(matches!(
        expand("{date/x", &vars()),
        Err(VisualError::Pattern(_))
    ));
    assert!(matches!(expand("", &vars()), Err(VisualError::Pattern(_))));
    assert!(validate_pattern("{date}/{breakpoint}").is_ok());
    assert!(validate_pattern("{date}/{oops}").is_err());
}

#[test]
fn segments_cannot_leave_the_folder() {
    let mut vars = vars();
    vars.breakpoint = "../../etc/passwd".to_owned();
    let path = expand("{date}/{breakpoint}", &vars).expect("valid");
    assert_eq!(path.components().count(), 2);
    assert!(
        path.components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
    );
    // A literal `..` or an absolute start is no escape either.
    let path = expand("../{breakpoint}", &self::vars()).expect("valid");
    assert_eq!(path, PathBuf::from("_").join("mobile"));
    let path = expand("/{breakpoint}", &self::vars()).expect("valid");
    assert_eq!(path, PathBuf::from("mobile"));
}

#[test]
fn sanitize_keeps_safe_characters_only() {
    assert_eq!(sanitize_segment("Desktop L"), "Desktop-L");
    assert_eq!(sanitize_segment("a:b*c?d\"e<f>g|h\\i"), "a-b-c-d-e-f-g-h-i");
    assert_eq!(sanitize_segment("..."), "_");
    assert_eq!(sanitize_segment(""), "_");
    assert_eq!(sanitize_segment("name. "), "name");
    assert_eq!(sanitize_segment("CON"), "_CON");
    assert_eq!(sanitize_segment("lpt1"), "_lpt1");
    assert_eq!(sanitize_segment("Gr\u{f6}\u{df}e"), "Gr-e");
    assert_eq!(sanitize_segment(&"a".repeat(200)).len(), MAX_SEGMENT);
}

#[test]
fn dpr_text_drops_trailing_zeros() {
    assert_eq!(dpr_text(3.0), "3x");
    assert_eq!(dpr_text(1.5), "1.5x");
    assert_eq!(dpr_text(2.625), "2.625x");
    assert_eq!(dpr_text(0.0), "");
}

#[test]
fn size_text_is_width_by_height_at_dpr() {
    assert_eq!(size_text(393, 852, 3.0), "393 \u{d7} 852 @3x");
    assert_eq!(size_text(1440, 900, 0.0), "1440 \u{d7} 900");
}
