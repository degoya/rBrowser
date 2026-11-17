use std::fs;
use std::path::Path;

use super::*;
use crate::video::registry::FileStamp;

const RUN: &str = "2026-11-16_14-03-07";

fn project_with(bytes: &[u8]) -> (tempfile::TempDir, Recorded) {
    let project = tempfile::tempdir().expect("temp dir");
    let dir = project.path().join(".rbrowser/out/video").join(RUN);
    fs::create_dir_all(&dir).expect("folder");
    fs::write(dir.join("mobile.mp4"), bytes).expect("file");
    fs::write(dir.join("notes.txt"), b"text").expect("file");
    let stamp =
        FileStamp::of(&fs::metadata(dir.join("mobile.mp4")).expect("metadata")).expect("stamp");
    let mut recorded = Recorded::default();
    recorded
        .recordings
        .entry(RUN.to_owned())
        .or_default()
        .insert("mobile.mp4".to_owned(), stamp);
    (project, recorded)
}

fn body(served: &mut Served) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buffer = [0_u8; 3];
    loop {
        let read = served.read(&mut buffer).expect("reads");
        if read == 0 {
            return out;
        }
        out.extend_from_slice(&buffer[..read]);
    }
}

fn url() -> String {
    url_of(RUN, "mobile.mp4")
}

#[test]
fn the_route_names_a_recording_and_a_video_file() {
    assert_eq!(url(), "rbrowser://video/2026-11-16_14-03-07/mobile.mp4");
    assert_eq!(
        parse_url(&url()),
        Some((RUN.to_owned(), "mobile.mp4".to_owned()))
    );
    assert_eq!(
        parse_url("RBROWSER://VIDEO/2026-11-16_14-03-07/500x800%402.mp4?t=1#x"),
        Some((RUN.to_owned(), "500x800@2.mp4".to_owned())),
        "case of scheme and host, an escaped name, query and fragment"
    );
    for bad in [
        "rbrowser://video/",
        "rbrowser://video/2026-11-16_14-03-07",
        "rbrowser://video/2026-11-16_14-03-07/",
        "rbrowser://video/2026-11-16_14-03-07/a/mobile.mp4",
        "rbrowser://video/../mobile.mp4",
        "rbrowser://video/2026-11-16_14-03-07/..%2Fmobile.mp4",
        "rbrowser://video/2026-11-16_14-03-07/%2e%2e",
        "rbrowser://video/2026-11-16_14-03-07/%ZZ.mp4",
        "rbrowser://video/2026-11-16_14-03-07/C%3A%5Cx.mp4",
        "rbrowser://video/2026-11-16_14-03-07/notes.txt",
        "rbrowser://video//etc/passwd",
        "rbrowser://ui/2026-11-16_14-03-07/mobile.mp4",
        "rbrowser://plugin/video/mobile.mp4",
        "file:///C:/x/mobile.mp4",
    ] {
        assert_eq!(parse_url(bad), None, "{bad}");
    }
}

/// Only a page of the shell in its main frame reaches the route - never a page in a view, a
/// frame in a page or a plugin panel.
#[test]
fn only_a_shell_page_may_ask() {
    assert!(requester_allowed(
        Some("rbrowser://ui/settings.html#/panels/videos"),
        true
    ));
    for (url, main) in [
        (Some("rbrowser://ui/settings.html"), false),
        (Some("https://example.com/"), true),
        (Some("rbrowser://plugin/hello-panel/panel/index.html"), true),
        (
            Some("rbrowser://video/2026-11-16_14-03-07/mobile.mp4"),
            true,
        ),
        (Some("about:blank"), true),
        (None, true),
    ] {
        assert!(!requester_allowed(url, main), "{url:?} main={main}");
    }
}

#[test]
fn ranges_follow_rfc_9110_for_one_range() {
    use ByteRange::{Full, Part, Unsatisfiable};
    let part = |start, end| Part { start, end };
    assert_eq!(parse_range(None, 100), Full);
    assert_eq!(parse_range(Some(""), 100), Full);
    assert_eq!(parse_range(Some("bytes=0-"), 100), part(0, 99));
    assert_eq!(parse_range(Some("bytes=10-19"), 100), part(10, 19));
    assert_eq!(parse_range(Some("Bytes = 10-19"), 100), part(10, 19));
    assert_eq!(
        parse_range(Some("bytes=90-500"), 100),
        part(90, 99),
        "the end is clipped"
    );
    assert_eq!(
        parse_range(Some("bytes=-10"), 100),
        part(90, 99),
        "a suffix"
    );
    assert_eq!(parse_range(Some("bytes=-500"), 100), part(0, 99));
    assert_eq!(
        parse_range(Some("items=0-1"), 100),
        Full,
        "another unit is ignored"
    );
    assert_eq!(
        parse_range(Some("bytes=0-1,5-6"), 100),
        Full,
        "several ranges: all of it"
    );
    // A header that does not read as a range is ignored (RFC 9110, 14.2): the whole file.
    for malformed in [
        "bytes",
        "0-10",
        "bytes=",
        "bytes=-",
        "bytes=a-b",
        "bytes=1-2-3",
        "bytes=+1-2",
        "bytes=20-10",
        "bytes=99999999999999999999999-",
        "bytes=-99999999999999999999999",
    ] {
        assert_eq!(parse_range(Some(malformed), 100), Full, "{malformed}");
    }
    // A range that reads but the file does not have.
    for unsatisfiable in ["bytes=100-", "bytes=150-160", "bytes=-0"] {
        assert_eq!(
            parse_range(Some(unsatisfiable), 100),
            Unsatisfiable,
            "{unsatisfiable}"
        );
    }
    assert_eq!(
        parse_range(Some("bytes=0-"), 0),
        Unsatisfiable,
        "an empty file has no range"
    );
    assert_eq!(parse_range(None, 0), Full);
    assert_eq!(parse_range(Some("bytes=x"), 0), Full);
}

#[test]
fn a_whole_file_is_served_with_its_type_and_length() {
    let (project, recorded) = project_with(b"0123456789");
    let mut served = Served::open(project.path(), &recorded, &url(), None).expect("served");
    assert_eq!(served.status(), (200, "OK"));
    assert_eq!(served.mime(), "video/mp4");
    assert_eq!(served.length(), 10);
    let headers = served.headers();
    assert!(headers.contains(&("Accept-Ranges", "bytes".to_owned())));
    assert!(headers.contains(&("X-Content-Type-Options", "nosniff".to_owned())));
    assert!(!headers.iter().any(|(name, _)| *name == "Content-Range"));
    assert_eq!(body(&mut served), b"0123456789");
}

#[test]
fn a_range_is_served_as_partial_content() {
    let (project, recorded) = project_with(b"0123456789");
    let mut served =
        Served::open(project.path(), &recorded, &url(), Some("bytes=2-5")).expect("served");
    assert_eq!(served.status(), (206, "Partial Content"));
    assert_eq!(served.length(), 4);
    assert!(
        served
            .headers()
            .contains(&("Content-Range", "bytes 2-5/10".to_owned()))
    );
    assert_eq!(body(&mut served), b"2345");
}

/// CEF skips to the range's first byte itself (`StreamReaderURLLoader`) - or does not: the
/// answer is the same either way.
#[test]
fn the_loaders_own_skip_to_the_range_start_is_absorbed() {
    let (project, recorded) = project_with(b"0123456789");
    let mut served =
        Served::open(project.path(), &recorded, &url(), Some("bytes=4-")).expect("served");
    assert_eq!(served.skip(4).expect("skips"), 4);
    assert_eq!(body(&mut served), b"456789");
    let mut unskipped =
        Served::open(project.path(), &recorded, &url(), Some("bytes=4-")).expect("served");
    assert_eq!(body(&mut unskipped), b"456789");
    let mut whole = Served::open(project.path(), &recorded, &url(), None).expect("served");
    assert_eq!(
        whole.skip(3).expect("skips"),
        3,
        "a skip without a range moves"
    );
    assert_eq!(whole.skip(50).expect("skips"), 7, "never past the end");
    assert_eq!(body(&mut whole), b"");
}

#[test]
fn an_unsatisfiable_range_answers_416_with_the_size() {
    let (project, recorded) = project_with(b"0123456789");
    let mut served =
        Served::open(project.path(), &recorded, &url(), Some("bytes=10-")).expect("answered");
    assert_eq!(served.status(), (416, "Range Not Satisfiable"));
    assert_eq!(served.length(), 0);
    assert!(
        served
            .headers()
            .contains(&("Content-Range", "bytes */10".to_owned()))
    );
    assert_eq!(served.skip(10).expect("skips"), 0);
    assert_eq!(body(&mut served), b"");
}

#[test]
fn nothing_but_a_video_rbrowser_recorded_in_the_open_project_is_served() {
    let (project, recorded) = project_with(b"0123456789");
    let other = tempfile::tempdir().expect("temp dir");
    for (root, url) in [
        (project.path(), format!("rbrowser://video/{RUN}/notes.txt")),
        (project.path(), format!("rbrowser://video/{RUN}/tablet.mp4")),
        (
            project.path(),
            "rbrowser://video/2020-01-01_00-00-00/mobile.mp4".to_owned(),
        ),
        (project.path(), "rbrowser://video/../mobile.mp4".to_owned()),
        (other.path(), url()),
    ] {
        assert!(
            Served::open(root, &recorded, &url, None).is_err(),
            "{url} in {}",
            root.display()
        );
    }
    assert!(Served::open(Path::new("/no/such/project"), &recorded, &url(), None).is_err());
    assert!(
        Served::open(project.path(), &Recorded::default(), &url(), None).is_err(),
        "a file rBrowser did not record is never served"
    );
    fs::write(
        project
            .path()
            .join(".rbrowser/out/video")
            .join(RUN)
            .join("mobile.mp4"),
        b"changed since",
    )
    .expect("changes");
    assert!(
        Served::open(project.path(), &recorded, &url(), None).is_err(),
        "nor one changed since"
    );
}

/// CEF's `Skip` (review RB-110-12): Chromium's player starts with `Range: bytes=0-`, and CEF asks
/// to skip 0 bytes - done at once, not "later" and not a failure.
#[test]
fn a_skip_of_nothing_is_done_and_one_that_moves_nothing_fails() {
    assert_eq!(skip_answer(0, Some(0)), (1, 0));
    assert_eq!(skip_answer(4, Some(4)), (1, 4));
    assert_eq!(
        skip_answer(10, Some(3)),
        (1, 3),
        "short of the end: what was skipped"
    );
    assert_eq!(
        skip_answer(10, Some(0)),
        (0, i64::from(ERR_FAILED)),
        "past the end"
    );
    assert_eq!(
        skip_answer(4, None),
        (0, i64::from(ERR_FAILED)),
        "the file failed"
    );
    assert_eq!(skip_answer(-1, Some(0)), (0, i64::from(ERR_FAILED)));
}

/// A video's poster (acceptance 2026-11-16) comes over the same route, as a JPEG, under the same
/// rules: registered, unchanged, no other image.
#[test]
fn a_registered_poster_is_served_as_a_jpeg_and_nothing_else_is() {
    let (project, mut recorded) = project_with(b"0123456789");
    let dir = project.path().join(".rbrowser/out/video").join(RUN);
    fs::write(dir.join("mobile.jpg"), b"jpeg").expect("file");
    fs::write(dir.join("other.png"), b"png").expect("file");
    let stamp =
        FileStamp::of(&fs::metadata(dir.join("mobile.jpg")).expect("metadata")).expect("stamp");
    let poster = url_of(RUN, "mobile.jpg");
    assert_eq!(
        parse_url(&poster),
        Some((RUN.to_owned(), "mobile.jpg".to_owned()))
    );
    assert_eq!(parse_url(&url_of(RUN, "other.png")), None);
    assert!(
        Served::open(project.path(), &recorded, &poster, None).is_err(),
        "not registered"
    );
    recorded
        .recordings
        .entry(RUN.to_owned())
        .or_default()
        .insert("mobile.jpg".to_owned(), stamp);
    let mut served = Served::open(project.path(), &recorded, &poster, None).expect("served");
    assert_eq!(served.status(), (200, "OK"));
    assert_eq!(served.mime(), "image/jpeg");
    assert_eq!(body(&mut served), b"jpeg");
}
