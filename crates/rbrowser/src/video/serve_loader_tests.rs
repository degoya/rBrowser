//! The route end to end as CEF and Chromium's media player use it (RB-110-12, acceptance
//! 2026-11-16: no WebM played). The loader below does with a [`Served`] what CEF 156 does with the
//! resource handler (`libcef/browser/net_service/stream_reader_url_loader.cc`:
//! `ParseRange`, `OnInputStreamOpened` skips to the range's first byte - not at all for 0 -,
//! `OnReaderSkipCompleted` fails unless exactly that many were skipped and takes the length from
//! a closed range, else from the handler; `resource_handler_wrapper.cc`: `GetResponseHeaders`
//! keeps that length), and checks what Chromium's media loader requires of the answer for a
//! scheme outside the HTTP family
//! (`third_party/blink/renderer/platform/media/resource_multi_buffer_data_provider.cc`,
//! `DidReceiveResponse`: the length is `content length + the request's start` and must match the
//! length seen before; `DidFinishLoading`: fewer bytes than that is a failure).

use std::fs;

use super::*;
use crate::video::registry::FileStamp;

const RUN: &str = "2026-11-16_14-03-07";
const NAME: &str = "ipad-mini.webm";

/// A file of `total` bytes, each byte its position modulo 251.
fn project(total: usize) -> (tempfile::TempDir, Recorded, Vec<u8>) {
    let project = tempfile::tempdir().expect("temp dir");
    let dir = project.path().join(".rbrowser/out/video").join(RUN);
    fs::create_dir_all(&dir).expect("folder");
    let bytes: Vec<u8> = (0..total)
        .map(|at| u8::try_from(at % 251).expect("below 251"))
        .collect();
    fs::write(dir.join(NAME), &bytes).expect("file");
    let stamp = FileStamp::of(&fs::metadata(dir.join(NAME)).expect("metadata")).expect("stamp");
    let mut recorded = Recorded::default();
    recorded
        .recordings
        .entry(RUN.to_owned())
        .or_default()
        .insert(NAME.to_owned(), stamp);
    (project, recorded, bytes)
}

/// `net::HttpUtil::ParseRangeHeader` for the one range CEF uses: `(first, last)`, `-1` for an
/// open end; `None` without a header or with several ranges.
fn cef_range(header: Option<&str>) -> Option<(i64, i64)> {
    let spec = header?.strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (first, last) = spec.split_once('-')?;
    let first: i64 = first.parse().ok()?;
    let last: i64 = if last.is_empty() {
        -1
    } else {
        last.parse().ok()?
    };
    Some((first, last))
}

/// What reached the media player.
#[derive(Debug)]
struct Delivered {
    status: i32,
    content_length: i64,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

/// One request through CEF's loader; `Err` is the net error CEF completes the request with.
fn cef_load(
    project: &tempfile::TempDir,
    recorded: &Recorded,
    range: Option<&str>,
    pipe: i32,
) -> Result<Delivered, i64> {
    let url = url_of(RUN, NAME);
    let mut served = Served::open(project.path(), recorded, &url, range).map_err(|_| -2_i64)?;
    let mut content_length = -1_i64;
    if let Some((first, last)) = cef_range(range) {
        // `InputStreamReader::SkipOnWorkThread`: nothing to skip never reaches the handler.
        let mut skipped = 0_i64;
        let mut left = first;
        while left > 0 {
            let moved = served.skip(u64::try_from(left).expect("positive")).ok();
            let (answer, count) = skip_answer(left, moved);
            if answer == 0 || count <= 0 {
                // `SkipToRequestedRange`: ERR_REQUEST_RANGE_NOT_SATISFIABLE.
                return Err(-328);
            }
            skipped += count;
            left -= count;
        }
        if skipped != first {
            return Err(-2);
        }
        if last >= 0 {
            content_length = last - first + 1;
        }
    }
    // `ResourceResponseWrapper::GetResponseHeaders`: the handler's length only without one.
    if content_length == -1 {
        content_length = i64::try_from(served.length()).expect("fits");
    }
    let mut body = Vec::new();
    loop {
        match served.read_for_loader(pipe) {
            Ok(chunk) if chunk.is_empty() => break,
            Ok(chunk) => {
                assert!(chunk.len() <= usize::try_from(pipe).expect("positive"));
                body.extend_from_slice(&chunk);
            }
            Err(_) => return Err(-2),
        }
    }
    Ok(Delivered {
        status: served.status().0,
        content_length,
        headers: served.headers(),
        body,
    })
}

/// Chromium's media loader for a scheme outside the HTTP family: the answer at `start` is
/// complete, its length adds up to `total`, its bytes are the file's from `start`.
fn media_accepts(delivered: &Delivered, start: usize, file: &[u8]) {
    let total = i64::try_from(file.len()).expect("fits");
    let start_at = i64::try_from(start).expect("fits");
    assert_eq!(
        delivered.content_length + start_at,
        total,
        "resolved length = content length + start"
    );
    assert_eq!(
        i64::try_from(delivered.body.len()).expect("fits"),
        delivered.content_length,
        "never fewer bytes than promised (DidFinishLoading)"
    );
    assert_eq!(delivered.body.as_slice(), &file[start..]);
}

#[test]
fn the_players_first_request_from_byte_0_gets_the_whole_file() {
    let (project, recorded, file) = project(3 * 1024 * 1024 + 17);
    let first = cef_load(&project, &recorded, Some("bytes=0-"), 64 * 1024).expect("answered");
    assert_eq!(first.status, 206);
    media_accepts(&first, 0, &file);
    let end = file.len() - 1;
    assert!(
        first
            .headers
            .contains(&("Content-Range", format!("bytes 0-{end}/{}", file.len())))
    );
    assert!(
        first
            .headers
            .contains(&("Accept-Ranges", "bytes".to_owned()))
    );
}

/// The WebM demuxer reads the head, then seeks to the cues near the end, then back into the
/// middle: each a new request `bytes=N-`, each complete and consistent.
#[test]
fn the_seeks_to_the_cues_and_back_are_each_consistent() {
    let (project, recorded, file) = project(3 * 1024 * 1024 + 17);
    let total = file.len();
    for start in [0, total - 4096, total - 1, 1_500_000, 1] {
        let header = format!("bytes={start}-");
        // Pipes of every size: CEF's data pipe, smaller and larger than the handler's chunk.
        for pipe in [1, 4096, 64 * 1024, i32::MAX] {
            let delivered = cef_load(&project, &recorded, Some(&header), pipe).expect("answered");
            assert_eq!(delivered.status, 206, "{header}");
            media_accepts(&delivered, start, &file);
            assert!(
                delivered.headers.contains(&(
                    "Content-Range",
                    format!("bytes {start}-{}/{total}", total - 1)
                )),
                "{header}"
            );
        }
    }
}

#[test]
fn a_request_without_a_range_gets_200_with_the_length_and_ranges_offered() {
    let (project, recorded, file) = project(70_000);
    let whole = cef_load(&project, &recorded, None, 64 * 1024).expect("answered");
    assert_eq!(whole.status, 200);
    media_accepts(&whole, 0, &file);
    assert!(
        whole
            .headers
            .contains(&("Accept-Ranges", "bytes".to_owned()))
    );
    assert!(
        !whole
            .headers
            .iter()
            .any(|(name, _)| *name == "Content-Range")
    );
}

#[test]
fn a_closed_range_inside_the_file_is_exactly_that_range() {
    let (project, recorded, file) = project(70_000);
    let part = cef_load(&project, &recorded, Some("bytes=100-199"), 64 * 1024).expect("answered");
    assert_eq!(part.status, 206);
    assert_eq!(part.content_length, 100);
    assert_eq!(part.body.as_slice(), &file[100..200]);
}

/// Several requests of one video at once (the head still streaming while the demuxer seeks):
/// each has its own file and position.
#[test]
fn overlapping_requests_do_not_disturb_each_other() {
    let (project, recorded, file) = project(200_000);
    let url = url_of(RUN, NAME);
    let mut head = Served::open(project.path(), &recorded, &url, Some("bytes=0-")).expect("open");
    let mut tail =
        Served::open(project.path(), &recorded, &url, Some("bytes=150000-")).expect("open");
    assert_eq!(tail.skip(150_000).expect("skips"), 150_000);
    let mut from_head = head.read_for_loader(1000).expect("reads");
    let from_tail = tail.read_for_loader(1000).expect("reads");
    assert_eq!(from_head.as_slice(), &file[..1000]);
    assert_eq!(from_tail.as_slice(), &file[150_000..151_000]);
    from_head.clear();
    while let Ok(chunk) = head.read_for_loader(CHUNK_I32) {
        if chunk.is_empty() {
            break;
        }
        from_head.extend_from_slice(&chunk);
    }
    assert_eq!(from_head.as_slice(), &file[1000..]);
}

const CHUNK_I32: i32 = 256 * 1024;

#[test]
fn a_read_never_hands_more_than_asked_nor_more_than_a_chunk() {
    let (project, recorded, _file) = project(CHUNK + 10);
    let url = url_of(RUN, NAME);
    let mut served = Served::open(project.path(), &recorded, &url, None).expect("open");
    assert!(served.read_for_loader(0).expect("reads").is_empty());
    let big = served.read_for_loader(i32::MAX).expect("reads");
    assert!(!big.is_empty() && big.len() <= CHUNK);
    assert!(served.read_for_loader(-5).expect("reads").is_empty());
}

/// The answer the panel's whole-file fallback fetches (a CORS request from the shell's page):
/// readable there, and nowhere else.
#[test]
fn the_answer_lets_the_shell_page_read_it() {
    let (project, recorded, _file) = project(100);
    let served = Served::open(project.path(), &recorded, &url_of(RUN, NAME), None).expect("served");
    assert!(
        served
            .headers()
            .contains(&("Access-Control-Allow-Origin", "rbrowser://ui".to_owned()))
    );
    assert_eq!(
        format!("{SHELL_PAGE_ORIGIN}/"),
        crate::shell::SHELL_ORIGIN,
        "the shell's origin"
    );
}

/// What the log says of an answer: status, the range asked, the `Content-Range`, the length -
/// never a path.
#[test]
fn an_answer_describes_itself_for_the_log() {
    let (project, recorded, _file) = project(100);
    let url = url_of(RUN, NAME);
    let part = Served::open(project.path(), &recorded, &url, Some("bytes=10-")).expect("served");
    assert_eq!(part.content_range().as_deref(), Some("bytes 10-99/100"));
    let whole = Served::open(project.path(), &recorded, &url, None).expect("served");
    assert_eq!(whole.content_range(), None);
    let beyond = Served::open(project.path(), &recorded, &url, Some("bytes=100-")).expect("served");
    assert_eq!(beyond.content_range().as_deref(), Some("bytes */100"));
    assert_eq!(logged_range(Some("bytes=10-")), "bytes=10-");
    assert_eq!(logged_range(None), "-");
    assert_eq!(
        logged_range(Some(&"x".repeat(500))).chars().count(),
        LOGGED_RANGE_CHARS,
        "a long header is cut"
    );
    assert_eq!(logged_range(Some("bytes=1-\u{7}\n")), "bytes=1-??");
}
