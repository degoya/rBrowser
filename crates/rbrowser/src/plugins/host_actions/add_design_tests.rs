use std::fs;

use super::*;

fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x04, 0x4A, 0x46];
    bytes.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x11, 0x08]);
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    bytes
}

fn webp(chunk: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut bytes = b"RIFF\0\0\0\0WEBP".to_vec();
    bytes.extend_from_slice(chunk);
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn the_type_comes_from_the_extension_and_the_content_must_match_it() {
    assert_eq!(Kind::of("home.PNG"), Some(Kind::Png));
    assert_eq!(Kind::of("a.jpeg"), Some(Kind::Jpeg));
    assert_eq!(Kind::of("a.JPG"), Some(Kind::Jpeg));
    assert_eq!(Kind::of("a.webp"), Some(Kind::Webp));
    assert_eq!(Kind::of("a.svg"), None);
    assert_eq!(Kind::of("png"), None);
    assert_eq!(check(&png(393, 852), Kind::Png), Ok((393, 852)));
    assert_eq!(check(&jpeg(1440, 900), Kind::Jpeg), Ok((1440, 900)));
    // A JPEG named .png, and a text file named .png, are refused.
    assert_eq!(check(&jpeg(10, 10), Kind::Png), Err(Refusal::BadType));
    assert_eq!(check(b"<svg/>", Kind::Png), Err(Refusal::BadType));
    assert_eq!(check(&png(0, 10), Kind::Png), Err(Refusal::BadType));
}

#[test]
fn webp_sizes_are_read_from_each_chunk_kind() {
    // VP8 (lossy): the frame tag, the start code, then 14-bit sizes.
    let mut lossy = vec![0u8; 10];
    lossy[3..6].copy_from_slice(&[0x9D, 0x01, 0x2A]);
    lossy[6..8].copy_from_slice(&393u16.to_le_bytes());
    lossy[8..10].copy_from_slice(&852u16.to_le_bytes());
    assert_eq!(check(&webp(b"VP8 ", &lossy), Kind::Webp), Ok((393, 852)));
    // VP8L (lossless): the signature byte, then width-1 and height-1 in 14 bits each.
    let bits: u32 = (392) | (851 << 14);
    let mut lossless = vec![0x2F];
    lossless.extend_from_slice(&bits.to_le_bytes());
    assert_eq!(check(&webp(b"VP8L", &lossless), Kind::Webp), Ok((393, 852)));
    // VP8X (extended): flags, then width-1 and height-1 in 24 bits each.
    let mut extended = vec![0u8; 4];
    extended.extend_from_slice(&[0x87, 0x01, 0x00, 0x53, 0x03, 0x00]);
    assert_eq!(check(&webp(b"VP8X", &extended), Kind::Webp), Ok((392, 852)));
    assert_eq!(
        check(&webp(b"ALPH", &[0; 10]), Kind::Webp),
        Err(Refusal::BadType)
    );
}

#[test]
fn the_limits_are_the_overlays() {
    assert_eq!(check(&png(16_384, 1_024), Kind::Png), Ok((16_384, 1_024)));
    assert_eq!(
        check(&png(16_385, 10), Kind::Png),
        Err(Refusal::TooManyPixels)
    );
    // 4097 x 4097 is a little over 16 megapixels.
    assert_eq!(
        check(&png(4_097, 4_097), Kind::Png),
        Err(Refusal::TooManyPixels)
    );
    let mut big = png(10, 10);
    big.resize(MAX_BYTES + 1, 0);
    assert_eq!(check(&big, Kind::Png), Err(Refusal::TooLarge));
}

#[test]
fn a_name_is_kept_and_made_one_the_overlay_takes() {
    assert_eq!(
        design_name("home-1440@2x.png").as_deref(),
        Some("home-1440@2x.png")
    );
    assert_eq!(
        design_name("Startseite (Entwurf).webp").as_deref(),
        Some("Startseite (Entwurf).webp")
    );
    // A character outside a plain file name becomes `-`; no hidden file, no short name.
    assert_eq!(design_name("a&b#c.png").as_deref(), Some("a-b-c.png"));
    assert_eq!(design_name(".hidden.png").as_deref(), Some("hidden.png"));
    assert_eq!(design_name("PROGRA~1.png").as_deref(), Some("PROGRA-1.png"));
    assert_eq!(design_name("..png").as_deref(), Some("design.png"));
    assert_eq!(design_name("notes.txt"), None);
    assert_eq!(design_name("png"), None);
    let long = format!("{}.png", "x".repeat(400));
    let named = design_name(&long).expect("a name");
    assert_eq!(named.chars().count(), MAX_STEM_CHARS + 4);
}

#[test]
fn a_number_goes_before_the_hints_of_width_and_scale() {
    assert_eq!(numbered("mobile.png", 2), "mobile (2).png");
    assert_eq!(numbered("home-1440.png", 2), "home (2)-1440.png");
    assert_eq!(numbered("home-1440@2x.png", 3), "home (3)-1440@2x.png");
    assert_eq!(numbered("hero@2x.webp", 10), "hero (10)@2x.webp");
    // A `-<digit>` that is no width stays with the name.
    assert_eq!(numbered("v-2.png", 2), "v-2 (2).png");
}

#[test]
fn a_picked_file_is_read_up_to_the_limit() {
    let dir = tempfile::tempdir().expect("temp dir");
    let small = dir.path().join("a.png");
    fs::write(&small, png(10, 10)).expect("write");
    assert_eq!(read_picked(&small), Ok(png(10, 10)));
    let large = dir.path().join("b.png");
    fs::write(&large, vec![0u8; MAX_BYTES + 1]).expect("write");
    assert_eq!(read_picked(&large), Err(Refusal::TooLarge));
    assert_eq!(
        read_picked(&dir.path().join("none.png")),
        Err(Refusal::Unreadable)
    );
}

#[test]
fn a_copy_never_overwrites_and_numbers_a_different_file_of_the_same_name() {
    let dir = tempfile::tempdir().expect("temp dir");
    let designs = dir.path().join(".rbrowser").join("designs");
    let first = copy_into(&designs, "home-1440.png", &png(1440, 900)).expect("first");
    assert_eq!(
        first,
        Copied {
            name: "home-1440.png".into(),
            reused: false
        }
    );
    // The same bytes again: the file there is the design, nothing is written.
    let same = copy_into(&designs, "home-1440.png", &png(1440, 900)).expect("same");
    assert_eq!(
        same,
        Copied {
            name: "home-1440.png".into(),
            reused: true
        }
    );
    // Other bytes under the name: numbered, the first file untouched.
    let other = copy_into(&designs, "home-1440.png", &png(1440, 901)).expect("other");
    assert_eq!(
        other,
        Copied {
            name: "home (2)-1440.png".into(),
            reused: false
        }
    );
    assert_eq!(
        fs::read(designs.join("home-1440.png")).expect("read"),
        png(1440, 900)
    );
    assert_eq!(
        fs::read(designs.join("home (2)-1440.png")).expect("read"),
        png(1440, 901)
    );
    let third = copy_into(&designs, "home-1440.png", &png(1440, 902)).expect("third");
    assert_eq!(third.name, "home (3)-1440.png");
}

#[test]
fn add_checks_then_copies_into_the_projects_designs() {
    let project = tempfile::tempdir().expect("project");
    let outside = tempfile::tempdir().expect("outside");
    let picked = outside.path().join("Mobile Home.png");
    fs::write(&picked, png(393, 852)).expect("write");
    let added = add(project.path(), &picked).expect("added");
    assert_eq!(added.name, "Mobile Home.png");
    assert_eq!(added.original, "Mobile Home.png");
    assert!(!added.renamed());
    assert!(!added.reused);
    let copied = project.path().join(DESIGNS_DIR).join("Mobile Home.png");
    assert_eq!(fs::read(copied).expect("copied"), png(393, 852));
    // The same file again: it is there already, nothing is written.
    let again = add(project.path(), &picked).expect("again");
    assert_eq!(again.name, "Mobile Home.png");
    assert!(again.reused);

    let fake = outside.path().join("fake.png");
    fs::write(&fake, b"GIF89a").expect("write");
    let refused = add(project.path(), &fake).expect_err("not an image");
    assert_eq!(refused, ("fake.png".to_owned(), Refusal::BadType));
    let text = outside.path().join("notes.txt");
    fs::write(&text, b"x").expect("write");
    assert_eq!(
        add(project.path(), &text).expect_err("no image name").1,
        Refusal::BadType
    );
}

#[cfg(unix)]
#[test]
fn a_designs_folder_that_is_a_link_is_not_written_through() {
    let project = tempfile::tempdir().expect("project");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    fs::create_dir_all(project.path().join(".rbrowser")).expect("config dir");
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join(DESIGNS_DIR)).expect("link");
    let designs = project.path().join(DESIGNS_DIR);
    assert_eq!(
        copy_into(&designs, "a.png", &png(1, 1)),
        Err(Refusal::NotWritten)
    );
    assert!(
        fs::read_dir(elsewhere.path())
            .expect("read")
            .next()
            .is_none()
    );
}

#[cfg(unix)]
#[test]
fn a_config_folder_that_is_a_link_is_not_written_through() {
    let project = tempfile::tempdir().expect("project");
    let elsewhere = tempfile::tempdir().expect("elsewhere");
    std::os::unix::fs::symlink(elsewhere.path(), project.path().join(".rbrowser")).expect("link");
    let designs = project.path().join(DESIGNS_DIR);
    assert_eq!(
        copy_into(&designs, "a.png", &png(1, 1)),
        Err(Refusal::NotWritten)
    );
    assert!(
        fs::read_dir(elsewhere.path())
            .expect("read")
            .next()
            .is_none()
    );
}

#[test]
fn a_reserved_windows_name_is_never_the_copys() {
    for (picked, named) in [
        ("CON.png", "CON_.png"),
        ("nul.jpg", "nul_.jpg"),
        ("Aux.webp", "Aux_.webp"),
        ("prn.png", "prn_.png"),
        ("COM1.png", "COM1_.png"),
        ("lpt9.png", "lpt9_.png"),
        ("con.v2.png", "con_.v2.png"),
        ("CON .png", "CON_.png"),
    ] {
        assert_eq!(design_name(picked).as_deref(), Some(named), "{picked}");
    }
    // Only the whole first part counts.
    assert_eq!(design_name("console.png").as_deref(), Some("console.png"));
    assert_eq!(design_name("COM10.png").as_deref(), Some("COM10.png"));
    assert_eq!(design_name("COM0.png").as_deref(), Some("COM0.png"));
}
