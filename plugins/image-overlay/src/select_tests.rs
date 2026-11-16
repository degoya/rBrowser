use super::*;
use rb_plugin_api::Capability;
use rb_plugin_api::test_support::MemoryHost;

use crate::image::tests::png;

fn view(breakpoint: &str, width: f64, dpr: f64) -> ViewFacts {
    ViewFacts {
        breakpoint: breakpoint.to_owned(),
        width,
        dpr,
    }
}

/// The acceptance fixture: `designs/mobile.png` (393 px) and `designs/desktop.png` (1440 px).
fn fixture() -> MemoryHost {
    MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/designs/mobile.png", &png(393, 852))
        .with_file(".rbrowser/designs/desktop.png", &png(1440, 900))
}

fn mapping() -> BTreeMap<String, String> {
    BTreeMap::from([
        ("mobile".to_owned(), "designs/mobile.png".to_owned()),
        ("desktop".to_owned(), "designs/desktop.png".to_owned()),
    ])
}

fn chosen(choice: Choice) -> Chosen {
    match choice {
        Choice::Image(chosen) => chosen,
        other => panic!("no image: {other:?}"),
    }
}

#[test]
fn each_view_gets_its_mapped_image_and_an_unmapped_one_the_nearest() {
    let mut host = fixture();
    let mobile = chosen(choose(
        &mut host,
        &mapping(),
        true,
        &view("mobile", 375.0, 3.0),
    ));
    assert_eq!(mobile.path.as_str(), "designs/mobile.png");
    assert_eq!((mobile.info.width, mobile.info.height), (393, 852));
    let desktop = chosen(choose(
        &mut host,
        &mapping(),
        true,
        &view("desktop", 1440.0, 1.0),
    ));
    assert_eq!(desktop.path.as_str(), "designs/desktop.png");
    // Tablet (768) has no entry: 393 is 375 away, 1440 is 672.
    let tablet = chosen(choose(
        &mut host,
        &mapping(),
        true,
        &view("tablet", 768.0, 2.0),
    ));
    assert_eq!(tablet.path.as_str(), "designs/mobile.png");
    assert_eq!(tablet.bytes, png(393, 852).len());
    assert_eq!(tablet.hash, fingerprint(&png(393, 852)));
}

#[test]
fn a_mapped_path_outside_the_designs_folder_is_refused_not_replaced() {
    let mut host = fixture().with_file(".rbrowser/out/x.png", &png(375, 10));
    let designs = BTreeMap::from([("mobile".to_owned(), "out/x.png".to_owned())]);
    let Choice::Problem(problem) = choose(&mut host, &designs, true, &view("mobile", 375.0, 1.0))
    else {
        panic!("not refused");
    };
    assert_eq!(problem.code, "path_outside");
    assert_eq!(
        problem.params,
        vec![("path".to_owned(), "out/x.png".to_owned())]
    );
    assert!(host.logs.iter().any(|(level, _)| level == "warn"));
}

#[test]
fn a_mapped_file_that_is_missing_or_no_image_is_a_problem() {
    let mut host = fixture().with_file(".rbrowser/designs/fake.png", b"<svg/>");
    let designs = BTreeMap::from([
        ("mobile".to_owned(), "designs/gone.png".to_owned()),
        ("tablet".to_owned(), "designs/fake.png".to_owned()),
        ("desktop".to_owned(), "designs/notes.svg".to_owned()),
    ]);
    let code = |choice: Choice| match choice {
        Choice::Problem(problem) => problem.code,
        other => panic!("no problem: {other:?}"),
    };
    assert_eq!(
        code(choose(
            &mut host,
            &designs,
            true,
            &view("mobile", 375.0, 1.0)
        )),
        "not_found"
    );
    assert_eq!(
        code(choose(
            &mut host,
            &designs,
            true,
            &view("tablet", 768.0, 1.0)
        )),
        "bad_type"
    );
    assert_eq!(
        code(choose(
            &mut host,
            &designs,
            true,
            &view("desktop", 1440.0, 1.0)
        )),
        "bad_type"
    );
}

#[test]
fn an_export_scale_sizes_the_design_when_auto_scaling_is_on() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/designs/mobile@3x.png", &png(1179, 2556));
    let designs = BTreeMap::from([("mobile".to_owned(), "designs/mobile@3x.png".to_owned())]);
    let on = chosen(choose(
        &mut host,
        &designs,
        true,
        &view("mobile", 393.0, 3.0),
    ));
    assert_eq!(on.scale, 3);
    let off = chosen(choose(
        &mut host,
        &designs,
        false,
        &view("mobile", 393.0, 3.0),
    ));
    assert_eq!(off.scale, 1);
}

#[test]
fn the_nearest_width_counts_css_px_and_prefers_the_views_pixel_ratio() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/designs/home-393.png", &png(393, 800))
        .with_file(".rbrowser/designs/home-393@3x.png", &png(1179, 2400))
        .with_file(".rbrowser/designs/wide@2x.png", &png(2880, 1800))
        .with_file(".rbrowser/designs/notes.txt", b"x")
        .with_file(".rbrowser/designs/sub/inner.png", &png(800, 10));
    let none = BTreeMap::new();
    let dpr3 = chosen(choose(&mut host, &none, true, &view("mobile", 390.0, 3.0)));
    assert_eq!(dpr3.path.as_str(), "designs/home-393@3x.png");
    let dpr1 = chosen(choose(&mut host, &none, true, &view("mobile", 390.0, 1.0)));
    assert_eq!(dpr1.path.as_str(), "designs/home-393.png");
    // `wide@2x.png` is 1440 CSS px wide, though 2880 image px.
    let wide = chosen(choose(&mut host, &none, true, &view("", 1400.0, 1.0)));
    assert_eq!(wide.path.as_str(), "designs/wide@2x.png");
    // A free-size view without a breakpoint takes the nearest as well; folders are not searched.
    let free = chosen(choose(&mut host, &none, true, &view("", 800.0, 1.0)));
    assert_eq!(free.path.as_str(), "designs/home-393.png");
}

#[test]
fn without_designs_there_is_no_image() {
    let mut host = MemoryHost::new().grant(Capability::ProjectRead);
    assert!(matches!(
        choose(
            &mut host,
            &BTreeMap::new(),
            true,
            &view("mobile", 375.0, 1.0)
        ),
        Choice::None
    ));
    let mut ungranted = MemoryHost::new();
    assert!(matches!(
        choose(
            &mut ungranted,
            &BTreeMap::new(),
            true,
            &view("mobile", 375.0, 1.0)
        ),
        Choice::None
    ));
}

#[test]
fn unusable_files_are_skipped_in_the_search() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/designs/broken-375.png", b"nope")
        .with_file(".rbrowser/designs/good-1440.png", &png(1440, 900));
    let picked = chosen(choose(
        &mut host,
        &BTreeMap::new(),
        true,
        &view("m", 375.0, 1.0),
    ));
    assert_eq!(picked.path.as_str(), "designs/good-1440.png");
}

#[test]
fn a_chunk_is_read_against_the_fingerprint() {
    let bytes = png(393, 852);
    let mut host = fixture();
    let path = DesignPath::parse("designs/mobile.png").expect("path");
    let hash = fingerprint(&bytes);
    assert_eq!(read_checked(&mut host, &path, &hash).expect("read"), bytes);
    let changed = read_checked(&mut host, &path, "0000000000000000").expect_err("changed");
    assert_eq!(changed.code, "changed");
}

/// The acceptance fixture as it lies in the repository (`tests/fixtures/project/.rbrowser/designs/`).
const MOBILE: &[u8] =
    include_bytes!("../../../tests/fixtures/project/.rbrowser/designs/mobile.png");
const DESKTOP: &[u8] =
    include_bytes!("../../../tests/fixtures/project/.rbrowser/designs/desktop.png");

#[test]
fn the_fixture_projects_designs_go_to_their_views_and_the_tablet_takes_the_nearest() {
    let mut host = MemoryHost::new()
        .grant(Capability::ProjectRead)
        .with_file(".rbrowser/designs/mobile.png", MOBILE)
        .with_file(".rbrowser/designs/desktop.png", DESKTOP);
    // The fixture project's breakpoints: mobile 375, tablet 768, desktop 1440.
    let mobile = chosen(choose(
        &mut host,
        &mapping(),
        true,
        &view("mobile", 375.0, 3.0),
    ));
    assert_eq!((mobile.info.width, mobile.info.height), (393, 852));
    let desktop = chosen(choose(
        &mut host,
        &mapping(),
        true,
        &view("desktop", 1440.0, 1.0),
    ));
    assert_eq!((desktop.info.width, desktop.info.height), (1440, 900));
    let tablet = chosen(choose(
        &mut host,
        &mapping(),
        true,
        &view("tablet", 768.0, 2.0),
    ));
    assert_eq!(tablet.path.as_str(), "designs/mobile.png");
    assert_eq!(tablet.bytes, MOBILE.len());
}
