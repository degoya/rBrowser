use super::*;

fn code(failure: &Failure) -> &str {
    failure.code().unwrap_or_default()
}

#[test]
fn a_png_below_the_out_folder_opens() {
    for path in [
        ".rbrowser/out/screenshots/2026-11-12_10-00-00/mobile.png",
        ".rbrowser/out/a.PNG",
        ".rbrowser/out/checks/run/x.png",
    ] {
        assert_eq!(source(path), Ok(path.to_owned()), "{path}");
    }
}

#[test]
fn anything_outside_the_out_folder_or_in_the_own_folder_is_refused() {
    for path in [
        "",
        "shot.png",
        ".rbrowser/designs/home.png",
        ".rbrowser/config.json",
        ".rbrowser/out/../config.local.png",
        ".rbrowser/out/./a.png",
        ".rbrowser/out//a.png",
        "/etc/a.png",
        ".rbrowser\\out\\a.png",
        "C:/x/.rbrowser/out/a.png",
        ".rbrowser/out/screenshot-editor/a.edited.png",
        ".rbrowser/out/a\u{7}.png",
    ] {
        let refused = source(path).expect_err(path);
        assert_eq!(
            code(&refused),
            "plugin.screenshot-editor.errors.path_outside",
            "{path}"
        );
    }
    let long = format!(".rbrowser/out/{}.png", "a".repeat(MAX_PATH));
    assert!(source(&long).is_err());
}

#[test]
fn only_png_files_open() {
    for path in [
        ".rbrowser/out/a.jpg",
        ".rbrowser/out/png",
        ".rbrowser/out/.png",
    ] {
        let refused = source(path).expect_err(path);
        assert_eq!(
            code(&refused),
            "plugin.screenshot-editor.errors.not_png",
            "{path}"
        );
    }
}

#[test]
fn the_edit_mirrors_the_original_below_the_own_folder() {
    let targets = mirror(
        ".rbrowser/out/screenshots/2026-11-12_10-00-00/mobile.png",
        Format::Png,
    );
    assert_eq!(
        targets,
        Targets {
            image: "screenshots/2026-11-12_10-00-00/mobile.edited.png".to_owned(),
            annotations: "screenshots/2026-11-12_10-00-00/mobile.annotations.json".to_owned(),
            format: Format::Png,
        }
    );
    assert_eq!(
        mirror(".rbrowser/out/a.PNG", Format::Jpeg).image,
        "a.edited.jpg"
    );
    assert_eq!(
        Targets::project("a.edited.jpg"),
        ".rbrowser/out/screenshot-editor/a.edited.jpg"
    );
}

#[test]
fn a_named_target_lies_in_the_own_folder_and_names_its_format() {
    let png = custom(".rbrowser/out/screenshot-editor/bugs/login.png").expect("png");
    assert_eq!(png.image, "bugs/login.png");
    assert_eq!(png.annotations, "bugs/login.annotations.json");
    assert_eq!(png.format, Format::Png);
    assert_eq!(
        custom(".rbrowser/out/screenshot-editor/a.JPEG")
            .expect("jpeg")
            .format,
        Format::Jpeg
    );
    assert_eq!(
        custom(".rbrowser/out/screenshot-editor/a.jpg")
            .expect("jpg")
            .annotations,
        "a.annotations.json"
    );
}

#[test]
fn a_named_target_outside_the_own_folder_is_refused() {
    for out in [
        "login.png",
        "docs/login.png",
        ".rbrowser/out/screenshots/login.png",
        ".rbrowser/out/screenshot-editor/../screenshots/login.png",
        "/tmp/login.png",
        "../login.png",
        ".rbrowser/config.json",
    ] {
        let refused = custom(out).expect_err(out);
        assert_eq!(
            code(&refused),
            "plugin.screenshot-editor.errors.path_outside",
            "{out}"
        );
        assert_eq!(refused.kind, FailureKind::AccessDenied);
    }
    for out in [
        ".rbrowser/out/screenshot-editor/login.gif",
        ".rbrowser/out/screenshot-editor/.png",
        ".rbrowser/out/screenshot-editor/a.annotations.png",
    ] {
        assert_eq!(
            code(&custom(out).expect_err(out)),
            "plugin.screenshot-editor.errors.out_extension",
            "{out}"
        );
    }
}

#[test]
fn formats_read_their_settings_names() {
    assert_eq!(Format::parse("png"), Some(Format::Png));
    assert_eq!(Format::parse("jpeg"), Some(Format::Jpeg));
    assert_eq!(Format::parse("gif"), None);
    assert_eq!(Format::Jpeg.name(), "jpeg");
}
