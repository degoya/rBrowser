use super::*;

#[test]
fn a_path_below_designs_is_taken_with_or_without_the_config_folder() {
    let path = DesignPath::parse("designs/mobile.png").expect("a design");
    assert_eq!(path.as_str(), "designs/mobile.png");
    assert_eq!(path.project_path(), ".rbrowser/designs/mobile.png");
    assert_eq!(path.kind(), Kind::Png);
    let same = DesignPath::parse(".rbrowser/designs/mobile.png").expect("a design");
    assert_eq!(same, path);
    let nested = DesignPath::parse("designs/home/Desktop-1440@2x.JPG").expect("nested");
    assert_eq!(
        nested.project_path(),
        ".rbrowser/designs/home/Desktop-1440@2x.JPG"
    );
}

#[test]
fn a_path_outside_designs_is_refused() {
    for path in [
        "mobile.png",
        "flows/x.png",
        "../designs/x.png",
        "designs/../config.json",
        "designs/../../secret.png",
        "designs/./x.png",
        "/etc/x.png",
        "C:/x.png",
        "designs\\x.png",
        "designs//x.png",
        "designs/",
        ".rbrowser/out/x.png",
        "src/designs/x.png",
        "designs/.hidden/x.png",
        "designs/PROGRA~1/x.png",
    ] {
        assert_eq!(
            DesignPath::parse(path),
            Err(PathRefusal::Outside),
            "{path} is outside"
        );
    }
}

#[test]
fn a_path_of_another_type_is_refused() {
    for path in [
        "designs/x.svg",
        "designs/x.gif",
        "designs/x",
        "designs/x.png.js",
    ] {
        assert_eq!(
            DesignPath::parse(path),
            Err(PathRefusal::BadType),
            "{path} is no image"
        );
    }
}

#[test]
fn an_overlong_or_odd_path_is_refused() {
    let long = format!("designs/{}.png", "a".repeat(MAX_PATH_CHARS));
    assert_eq!(DesignPath::parse(&long), Err(PathRefusal::Outside));
    assert_eq!(
        DesignPath::parse("designs/a\u{0}b.png"),
        Err(PathRefusal::Outside)
    );
    assert_eq!(
        DesignPath::parse("designs/a:b.png"),
        Err(PathRefusal::Outside)
    );
    assert!(DesignPath::parse("designs/Home page (v2) +x.png").is_ok());
}

#[test]
fn a_listed_name_becomes_a_design_path() {
    assert_eq!(
        DesignPath::listed("tablet-768.webp").map(|path| path.as_str().to_owned()),
        Some("designs/tablet-768.webp".to_owned())
    );
    assert_eq!(DesignPath::listed("sub/"), None);
    assert_eq!(DesignPath::listed("notes.txt"), None);
}

#[test]
fn a_breakpoint_id_is_checked() {
    assert!(breakpoint_id_ok("mobile"));
    assert!(breakpoint_id_ok("tablet_2-x"));
    assert!(!breakpoint_id_ok(""));
    assert!(!breakpoint_id_ok("a b"));
    assert!(!breakpoint_id_ok(&"a".repeat(65)));
}
