use super::*;

/// Plain http is refused before any connection is made.
#[test]
fn the_http_fetcher_refuses_plain_http() {
    let Err(error) = HttpFetcher::new().open("http://127.0.0.1:9/ffmpeg.zip") else {
        panic!("plain http was opened");
    };
    assert!(error.contains("not https"), "{error}");
}

#[test]
fn the_memory_fetcher_serves_and_counts() {
    let fetcher = MemoryFetcher::default();
    fetcher.serve("https://example.test/a", b"abc".to_vec());
    let mut download = fetcher.open("https://example.test/a").expect("served");
    assert_eq!(download.length, Some(3));
    let mut body = Vec::new();
    download.body.read_to_end(&mut body).expect("read");
    assert_eq!(body, b"abc");
    assert!(fetcher.open("https://example.test/missing").is_err());
    assert_eq!(fetcher.requests().len(), 2);
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
    format!("http://{address}/ffmpeg.zip")
}

/// The agent of [`config`] without the https rule, so the local server can be reached.
fn local_agent(idle: Duration, body: Duration) -> ureq::Agent {
    config(idle, body).https_only(false).build().into()
}

fn read_body(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let response = agent.get(url).call().map_err(|error| error.to_string())?;
    let mut body = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| error.to_string())?;
    Ok(body)
}

/// Charge 9 review H1: a body that keeps arriving may take longer than the idle timeout - the
/// 168 MB ffmpeg on a slow line. `timeout_recv_body` alone was the whole body's budget.
#[test]
fn a_slow_body_that_keeps_coming_is_read_past_the_idle_timeout() {
    let url = slow_server(b"0123456789", Duration::from_millis(150), Duration::ZERO);
    let agent = local_agent(Duration::from_millis(600), Duration::from_secs(60));
    assert_eq!(read_body(&agent, &url).expect("read"), b"0123456789");
}

/// A body that stops for longer than the idle timeout is cut off.
#[test]
fn a_body_that_stalls_past_the_idle_timeout_fails() {
    let url = slow_server(b"0123", Duration::ZERO, Duration::from_secs(3));
    let agent = local_agent(Duration::from_millis(300), Duration::from_secs(60));
    assert!(read_body(&agent, &url).is_err());
}

/// The production budget: a minute without a byte, hours for the whole body.
#[test]
fn the_production_client_has_an_idle_and_a_generous_total_budget() {
    let config = config(READ_TIMEOUT, BODY_TIMEOUT).build();
    assert_eq!(config.timeout_per_read(), Some(READ_TIMEOUT));
    assert_eq!(config.timeouts().recv_body, Some(BODY_TIMEOUT));
    assert!(config.https_only());
    // The pinned ffmpeg (168 MB) at 200 kbit/s fits the total budget.
    assert!(BODY_TIMEOUT >= Duration::from_secs(168_580_960 * 8 / 200_000));
}
