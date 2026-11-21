use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};

use tracing_subscriber::fmt::MakeWriter;

use super::*;

/// A log written into memory, for the tests that read what adb's runs leave in it (RB-096-10).
#[derive(Clone, Default)]
pub(crate) struct Log(Arc<Mutex<Vec<u8>>>);

impl Log {
    /// What was written so far.
    pub(crate) fn text(&self) -> String {
        String::from_utf8_lossy(&crate::cdp::lock(&self.0)).into_owned()
    }

    /// A subscriber at DEBUG into this log, for `tracing::subscriber::set_default`.
    pub(crate) fn subscriber(&self) -> impl tracing::Subscriber + Send + Sync + use<> {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .with_ansi(false)
            .with_writer(self.clone())
            .finish()
    }
}

impl Write for Log {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        crate::cdp::lock(&self.0).extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Log {
    type Writer = Log;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// adb as the tests see it: an answer per command line, every call recorded.
#[derive(Default)]
pub(crate) struct FakeAdb {
    answers: Mutex<HashMap<String, Result<String, AdbError>>>,
    calls: Mutex<Vec<String>>,
}

impl FakeAdb {
    pub(crate) fn answer(&self, args: &[String], answer: Result<&str, AdbError>) {
        crate::cdp::lock(&self.answers).insert(args.join(" "), answer.map(ToOwned::to_owned));
    }

    pub(crate) fn calls(&self) -> Vec<String> {
        crate::cdp::lock(&self.calls).clone()
    }
}

#[async_trait]
impl AdbRun for FakeAdb {
    async fn run(&self, args: &[String]) -> Result<String, AdbError> {
        let line = args.join(" ");
        crate::cdp::lock(&self.calls).push(line.clone());
        crate::cdp::lock(&self.answers)
            .get(&line)
            .cloned()
            .unwrap_or_else(|| Err(AdbError::Failed(format!("no answer for {line}"))))
    }
}

#[tokio::test]
async fn the_devices_are_listed() {
    let adb = FakeAdb::default();
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nS1 device model:Pixel_8\n"),
    );
    let devices = devices(&adb).await.expect("listed");
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].name(), "Pixel 8");
}

// RB-096-10 and the w31 security review: the debug log names how many devices adb listed and
// each one's state, its serial cut to four characters - never the whole serial, the model or
// the USB path.
#[tokio::test]
async fn the_device_listing_is_in_the_debug_log() {
    let log = Log::default();
    let _guard = tracing::subscriber::set_default(log.subscriber());
    let adb = FakeAdb::default();
    adb.answer(
        &devices_args(),
        Ok("List of devices attached\nR5CX1234ABCD unauthorized usb:1-1 model:Pixel_8 transport_id:2\n\
            emulator-5554 device product:sdk model:Phone\n"),
    );
    let devices = devices(&adb).await.expect("listed");
    assert_eq!(devices.len(), 2);
    let text = log.text();
    assert!(text.contains("adb devices -l"), "{text}");
    assert!(text.contains("devices=2"), "{text}");
    assert!(text.contains("R5CX\u{2026} unauthorized"), "{text}");
    assert!(text.contains("emul\u{2026} device"), "{text}");
    for hidden in ["R5CX1234ABCD", "usb:1-1", "Pixel", "emulator-5554"] {
        assert!(!text.contains(hidden), "{hidden}: {text}");
    }
}

#[tokio::test]
async fn the_screen_is_read_from_wm() {
    let adb = FakeAdb::default();
    adb.answer(&screen_size_args("S1"), Ok("Physical size: 1080x2400\n"));
    adb.answer(&screen_density_args("S1"), Ok("Physical density: 420\n"));
    let read = screen(&adb, "S1").await.expect("a screen");
    assert_eq!(read.css_size(), (411, 914));
    adb.answer(&screen_density_args("S1"), Ok("no density"));
    assert!(matches!(
        screen(&adb, "S1").await,
        Err(AdbError::Unexpected(_))
    ));
}

#[tokio::test]
async fn a_forward_answers_its_port_and_a_bad_answer_is_an_error() {
    let adb = FakeAdb::default();
    adb.answer(&forward_args("S1"), Ok("51234\n"));
    assert_eq!(forward(&adb, "S1").await.expect("forwarded"), 51234);
    adb.answer(&forward_args("S1"), Ok("error: closed"));
    assert!(matches!(
        forward(&adb, "S1").await,
        Err(AdbError::Unexpected(_))
    ));
}

/// The serial goes to adb only when it is a plain USB serial.
#[tokio::test]
async fn a_serial_that_is_no_plain_serial_never_reaches_adb() {
    let adb = FakeAdb::default();
    for serial in ["-a", "host:5555", ""] {
        assert!(matches!(
            forward(&adb, serial).await,
            Err(AdbError::BadSerial)
        ));
        assert!(matches!(
            screen(&adb, serial).await,
            Err(AdbError::BadSerial)
        ));
        assert!(matches!(
            start_chrome(&adb, serial).await,
            Err(AdbError::BadSerial)
        ));
    }
    assert!(adb.calls().is_empty(), "{:?}", adb.calls());
}

#[test]
fn the_error_text_is_cut() {
    let long = "x".repeat(2000);
    let error = failure(Some(1), long.as_bytes());
    assert!(error.to_string().len() < 400, "{error}");
}

/// The exposure check's addresses: the one every probed route leaves from, each once.
#[test]
fn the_own_addresses_are_the_routes_sources_each_once() {
    let lan = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20));
    let vpn = IpAddr::V4(Ipv4Addr::new(100, 64, 0, 7));
    let source = |target: SocketAddr| match target.ip() {
        IpAddr::V4(ip) if ip.octets()[0] == 100 => Some(vpn),
        IpAddr::V4(_) => Some(lan),
        IpAddr::V6(_) => None,
    };
    assert_eq!(addresses_from(source), [lan, vpn]);
    assert!(
        ROUTE_PROBES.iter().any(|target| target.is_ipv6()),
        "IPv6 is asked too"
    );
    assert!(addresses_from(|_| None).is_empty());
}
