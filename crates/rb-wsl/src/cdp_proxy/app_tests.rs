use tokio::io::BufReader;

use super::*;
use crate::cdp_proxy::helper::serve_cdp_proxy;

#[tokio::test]
async fn the_handshake_with_a_real_helper_side_gives_its_port() {
    let (app, helper) = tokio::io::duplex(1 << 16);
    let (input, output) = tokio::io::split(helper);
    tokio::spawn(serve_cdp_proxy(BufReader::new(input), output, "1.0.0", 0));
    let (read, mut write) = tokio::io::split(app);
    let mut read = BufReader::new(read);
    let port = handshake(&mut read, &mut write).await.expect("a port");
    assert_ne!(port, 0);
}

#[tokio::test]
async fn noise_before_the_welcome_is_skipped_and_a_missing_port_is_an_error() {
    let (app, helper) = tokio::io::duplex(1 << 16);
    let (read, mut write) = tokio::io::split(app);
    let mut read = BufReader::new(read);
    let (helper_read, mut helper_write) = tokio::io::split(helper);
    tokio::spawn(async move {
        let mut helper_read = BufReader::new(helper_read);
        let mut hello_line = String::new();
        helper_read.read_line(&mut hello_line).await.expect("hello");
        let (welcome, _) = protocol::answer_hello(&hello_line, "1.0.0");
        let lines = format!(
            "wsl: a notice\n{}{}",
            protocol::encode(&welcome),
            frame::encode(&Frame::Ping)
        );
        helper_write
            .write_all(lines.as_bytes())
            .await
            .expect("writes");
    });
    let error = handshake(&mut read, &mut write).await.expect_err("no port");
    assert!(matches!(error, WslError::HelperStart(_)), "{error:?}");
}

#[tokio::test]
async fn a_helper_that_ends_at_once_fails_the_handshake() {
    let (app, helper) = tokio::io::duplex(1 << 16);
    drop(helper);
    let (read, mut write) = tokio::io::split(app);
    let mut read = BufReader::new(read);
    let error = handshake(&mut read, &mut write).await.expect_err("closed");
    assert!(matches!(error, WslError::Handshake(_)), "{error:?}");
}
