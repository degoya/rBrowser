//! Running `adb` (RB-095-02): [`AdbRun`] is one command line, [`AdbProgram`] the real program
//! (`.no_console_window()`, stdin closed, killed when dropped, at most [`RUN_TIMEOUT`] and
//! [`rb_process::MAX_OUTPUT`] bytes of output, RB-095-05); the
//! functions on top list the devices, read a screen, forward Chrome's socket and start Chrome.
//! A serial reaches adb only when it is a plain USB serial (`super::devices::valid_serial`).
//! The argument lists are `super::forward`'s and `super::reverse`'s, so a test holds that none
//! opens anything to the network.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, UdpSocket};
use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use rb_process::NoConsoleWindow as _;
use tokio::process::Command;

use super::devices::{Device, DeviceState, Screen, parse_devices, parse_screen, valid_serial};
use super::forward::{
    devices_args, forward_args, parse_port, remove_args, screen_density_args, screen_size_args,
    start_chrome_args,
};

/// How long one adb command may take (the first starts adb's server).
pub const RUN_TIMEOUT: Duration = Duration::from_secs(20);
/// How long the exposure check waits for a connection at a network address.
pub const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
/// The most characters of adb's error output kept in a message.
const MAX_MESSAGE: usize = 300;
/// The characters of a serial the debug log keeps (the w31 security review).
const SHOWN_SERIAL: usize = 4;
/// The most devices the debug log names one by one.
const MAX_LOGGED: usize = 16;

/// Why an adb command did not answer as expected.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdbError {
    /// The program did not start.
    #[error("adb could not be started: {0}")]
    Start(String),
    /// It did not end in time.
    #[error("adb did not answer within {} s", RUN_TIMEOUT.as_secs())]
    Timeout,
    /// It ended with an error.
    #[error("adb failed: {0}")]
    Failed(String),
    /// Its answer does not read.
    #[error("adb answered something unexpected: {0}")]
    Unexpected(String),
    /// The serial is no plain USB serial.
    #[error("that is no serial of a USB device")]
    BadSerial,
}

/// One adb command line.
#[async_trait]
pub trait AdbRun: Send + Sync {
    /// Runs `adb <args>` and answers its standard output.
    async fn run(&self, args: &[String]) -> Result<String, AdbError>;
}

/// The adb program.
#[derive(Clone, Debug)]
pub struct AdbProgram {
    path: PathBuf,
}

impl AdbProgram {
    /// adb at `path`.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl AdbRun for AdbProgram {
    async fn run(&self, args: &[String]) -> Result<String, AdbError> {
        let mut command = Command::new(&self.path);
        command
            .args(args)
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .no_console_window();
        let output = rb_process::output_capped(&mut command, rb_process::MAX_OUTPUT);
        let output = tokio::time::timeout(RUN_TIMEOUT, output)
            .await
            .map_err(|_| AdbError::Timeout)?
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::OutOfMemory => AdbError::Unexpected(error.to_string()),
                kind => AdbError::Start(kind.to_string()),
            })?;
        if !output.status.success() {
            return Err(failure(output.status.code(), &output.stderr));
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }
}

/// The error of a run that ended with `code`, with the start of what it wrote.
pub(crate) fn failure(code: Option<i32>, stderr: &[u8]) -> AdbError {
    let text = String::from_utf8_lossy(stderr);
    let text = rb_util::text::cut_ellipsis(text.trim(), MAX_MESSAGE);
    match code {
        Some(code) => AdbError::Failed(format!("exit {code}: {text}")),
        None => AdbError::Failed(text),
    }
}

fn checked(serial: &str) -> Result<(), AdbError> {
    valid_serial(serial)
        .then_some(())
        .ok_or(AdbError::BadSerial)
}

/// `adb devices -l`.
///
/// # Errors
///
/// adb did not run.
pub async fn devices(adb: &dyn AdbRun) -> Result<Vec<Device>, AdbError> {
    let answer = adb.run(&devices_args()).await?;
    let devices = parse_devices(&answer);
    // RB-096-10: what adb saw, for a device that does not show (driver, USB mode, Auto Blocker):
    // the count, each device's state and the lines not taken - never a whole serial, a model
    // or a USB path (the w31 security review).
    let listed = answer
        .lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() && !line.starts_with('*') && !line.starts_with("List of devices")
        })
        .count();
    tracing::debug!(
        devices = devices.len(),
        unread = listed.saturating_sub(devices.len()),
        states = %listing_note(&devices),
        "adb devices -l"
    );
    Ok(devices)
}

/// The devices as the debug log names them: the serial's first [`SHOWN_SERIAL`] characters, an
/// ellipsis and the state.
fn listing_note(devices: &[Device]) -> String {
    let mut named: Vec<String> = devices
        .iter()
        .take(MAX_LOGGED)
        .map(|device| {
            format!(
                "{} {}",
                shown_serial(&device.serial),
                state_word(&device.state)
            )
        })
        .collect();
    if devices.len() > MAX_LOGGED {
        named.push("\u{2026}".to_owned());
    }
    named.join(", ")
}

/// A serial as the debug log names it: its first [`SHOWN_SERIAL`] characters and an ellipsis.
pub(crate) fn shown_serial(serial: &str) -> String {
    let start: String = serial.chars().take(SHOWN_SERIAL).collect();
    format!("{start}\u{2026}")
}

/// A state as adb writes it; another one only by its letters, cut.
fn state_word(state: &DeviceState) -> String {
    match state {
        DeviceState::Device => "device".to_owned(),
        DeviceState::Unauthorized => "unauthorized".to_owned(),
        DeviceState::Offline => "offline".to_owned(),
        DeviceState::NoPermissions => "no permissions".to_owned(),
        DeviceState::Other(word) => word
            .chars()
            .filter(char::is_ascii_alphabetic)
            .take(20)
            .collect(),
    }
}

/// The screen of `serial`.
///
/// # Errors
///
/// A bad serial, adb did not run, or `wm` answered no size.
pub async fn screen(adb: &dyn AdbRun, serial: &str) -> Result<Screen, AdbError> {
    checked(serial)?;
    let size = adb.run(&screen_size_args(serial)).await?;
    let density = adb.run(&screen_density_args(serial)).await?;
    parse_screen(&size, &density).ok_or_else(|| AdbError::Unexpected("no screen size".to_owned()))
}

/// Forwards a free local port on 127.0.0.1 to Chrome's DevTools socket of `serial`.
///
/// # Errors
///
/// A bad serial, adb did not run, or it printed no port.
pub async fn forward(adb: &dyn AdbRun, serial: &str) -> Result<u16, AdbError> {
    checked(serial)?;
    let output = adb.run(&forward_args(serial)).await?;
    parse_port(&output).ok_or_else(|| AdbError::Unexpected("no forwarded port".to_owned()))
}

/// Removes the forward of `port`.
///
/// # Errors
///
/// A bad serial, or adb did not run.
pub async fn unforward(adb: &dyn AdbRun, serial: &str, port: u16) -> Result<(), AdbError> {
    checked(serial)?;
    adb.run(&remove_args(serial, port)).await.map(|_| ())
}

/// Starts Chrome on `serial`, so its DevTools socket exists.
///
/// # Errors
///
/// A bad serial, or adb did not run.
pub async fn start_chrome(adb: &dyn AdbRun, serial: &str) -> Result<(), AdbError> {
    checked(serial)?;
    adb.run(&start_chrome_args(serial)).await.map(|_| ())
}

const fn v4(a: u8, b: u8, c: u8, d: u8) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), 9))
}

const fn v6(first: u16, second: u16) -> SocketAddr {
    SocketAddr::V6(SocketAddrV6::new(
        Ipv6Addr::new(first, second, 0, 0, 0, 0, 0, 1),
        9,
        0,
        0,
    ))
}

/// The targets whose routes name this machine's interfaces: the default route (TEST-NET-1 of
/// RFC 5737 and the IPv6 documentation prefix, neither ever answers), the private ranges, the
/// shared range VPNs use and link-local, IPv4 and IPv6. Connecting a UDP socket sends nothing;
/// it only picks the interface the route leaves from.
pub(crate) const ROUTE_PROBES: [SocketAddr; 8] = [
    v4(192, 0, 2, 1),
    v4(10, 0, 0, 1),
    v4(172, 16, 0, 1),
    v4(192, 168, 0, 1),
    v4(100, 64, 0, 1),
    v4(169, 254, 0, 1),
    v6(0x2001, 0x0db8),
    v6(0xfd00, 0),
];

/// The address each of [`ROUTE_PROBES`] leaves from (`source`), each once, in their order.
pub(crate) fn addresses_from(source: impl Fn(SocketAddr) -> Option<IpAddr>) -> Vec<IpAddr> {
    let mut addresses = Vec::new();
    for ip in ROUTE_PROBES.iter().filter_map(|target| source(*target)) {
        if !addresses.contains(&ip) {
            addresses.push(ip);
        }
    }
    addresses
}

/// This machine's own network addresses that a forward must not answer at: the source of
/// every route [`ROUTE_PROBES`] asks for. Empty without a network - then the exposure check
/// refuses the forward (`super::forward::Exposed::Unchecked`).
pub fn own_addresses() -> Vec<IpAddr> {
    addresses_from(|target| {
        let unspecified: IpAddr = if target.is_ipv4() {
            Ipv4Addr::UNSPECIFIED.into()
        } else {
            Ipv6Addr::UNSPECIFIED.into()
        };
        let socket = UdpSocket::bind((unspecified, 0)).ok()?;
        socket.connect(target).ok()?;
        Some(socket.local_addr().ok()?.ip())
    })
}

/// Whether something accepts a connection at `addr` within [`PROBE_TIMEOUT`] (blocking).
pub fn answers(addr: SocketAddr) -> bool {
    std::net::TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok()
}

#[cfg(test)]
#[path = "adb_tests.rs"]
pub(crate) mod tests;
