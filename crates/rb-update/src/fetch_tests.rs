use super::*;

fn url(text: &str) -> Url {
    Url::parse(text).expect("url")
}

#[test]
fn the_memory_fetcher_serves_records_and_answers_404() {
    let fetcher = MemoryFetcher::new();
    fetcher.serve("https://example.test/a.json", b"{}".to_vec());
    assert_eq!(
        fetcher
            .fetch(&url("https://example.test/a.json"), 10)
            .expect("fetch"),
        Some(b"{}".to_vec())
    );
    assert_eq!(
        fetcher
            .fetch(&url("https://example.test/missing.json"), 10)
            .expect("fetch"),
        None
    );
    assert_eq!(
        fetcher.requests(),
        [
            "https://example.test/a.json",
            "https://example.test/missing.json"
        ]
    );
}

#[test]
fn a_body_over_the_limit_is_refused() {
    let fetcher = MemoryFetcher::new();
    fetcher.serve("https://example.test/big", vec![0_u8; 11]);
    let error = fetcher
        .fetch(&url("https://example.test/big"), 10)
        .expect_err("too large");
    assert!(error.downcast_ref::<BodyTooLarge>().is_some());
}

#[test]
fn plain_http_is_refused_before_anything_is_read() {
    let fetcher = MemoryFetcher::new();
    fetcher.serve("http://example.test/a.json", b"{}".to_vec());
    assert!(
        fetcher
            .fetch(&url("http://example.test/a.json"), 10)
            .is_err()
    );
    assert!(fetcher.open(&url("http://example.test/a.json")).is_err());
    // The network fetcher refuses it without a connection.
    let http = HttpFetcher::new();
    let error = http
        .fetch(&url("http://127.0.0.1:9/a.json"), 10)
        .expect_err("http");
    assert!(format!("{error:#}").contains("is not https"), "{error:#}");
}

#[test]
fn a_streamed_body_arrives_whole_in_small_chunks() {
    let fetcher = MemoryFetcher::new();
    let bytes: Vec<u8> = (0..100).collect();
    fetcher.serve("https://example.test/file.zip", bytes.clone());
    let mut download = fetcher
        .open(&url("https://example.test/file.zip"))
        .expect("open");
    assert_eq!(download.length, Some(100));
    let mut first = [0_u8; 64];
    assert_eq!(download.body.read(&mut first).expect("read"), 7);
    let mut rest = Vec::new();
    download.body.read_to_end(&mut rest).expect("read");
    assert_eq!([&first[..7], &rest[..]].concat(), bytes);
}

#[test]
fn the_user_agent_names_only_the_product_and_version() {
    assert_eq!(
        user_agent(),
        format!("rBrowser/{}", env!("CARGO_PKG_VERSION"))
    );
}

/// Serves one `200` of `body` on a local port, one byte at a time with `gap` between them;
/// `stall` more before the second byte. Answers the address.
fn slow_server(body: &'static [u8], gap: Duration, stall: Duration) -> String {
    use std::io::Write as _;
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
    let address = listener.local_addr().expect("address");
    std::thread::spawn(move || {
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut request = Vec::new();
        let mut byte = [0_u8; 1];
        while !request.ends_with(b"\r\n\r\n") && stream.read(&mut byte).is_ok_and(|n| n == 1) {
            request.push(byte[0]);
        }
        let head = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(head.as_bytes());
        for (index, byte) in body.iter().enumerate() {
            if index == 1 {
                std::thread::sleep(stall);
            }
            if stream
                .write_all(&[*byte])
                .and_then(|()| stream.flush())
                .is_err()
            {
                return;
            }
            std::thread::sleep(gap);
        }
    });
    format!("http://{address}/rbrowser.zip")
}

fn read_body(idle: Duration, address: &str) -> std::result::Result<Vec<u8>, String> {
    let agent: ureq::Agent = config(idle, Duration::from_secs(60))
        .https_only(false)
        .build()
        .into();
    let response = agent
        .get(address)
        .call()
        .map_err(|error| error.to_string())?;
    let mut body = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(body)
}

/// Charge 9 review H1: a body that keeps arriving may take longer than the idle timeout; one
/// that stops for longer is cut off.
#[test]
fn the_idle_timeout_is_per_read_not_for_the_whole_body() {
    let slow = slow_server(b"0123456789", Duration::from_millis(150), Duration::ZERO);
    assert_eq!(
        read_body(Duration::from_millis(600), &slow).expect("read"),
        b"0123456789"
    );
    let stalled = slow_server(b"0123", Duration::ZERO, Duration::from_secs(3));
    assert!(read_body(Duration::from_millis(300), &stalled).is_err());
    let production = config(
        Duration::from_secs(READ_TIMEOUT_SECONDS),
        Duration::from_secs(BODY_TIMEOUT_SECONDS),
    )
    .build();
    assert_eq!(
        production.timeout_per_read(),
        Some(Duration::from_secs(READ_TIMEOUT_SECONDS))
    );
    assert!(production.https_only());
}
