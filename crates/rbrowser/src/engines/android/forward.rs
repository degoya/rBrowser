//! The way to Chrome on the device (RB-095-02), pure: the adb calls rBrowser makes
//! ([`forward_args`] and the others), the port adb chose ([`parse_port`]), Chrome's browser
//! socket dialled on loopback only ([`browser_socket`]), the plain HTTP request for it
//! ([`version_request`], [`http_body`]) and the history step of back and forward
//! ([`history_entry`]).
//!
//! **Security (the job's review condition):** the forwarded port is `tcp:0` - adb chooses a
//! free one and binds it to `127.0.0.1` - and no call passes `-a` or `tcpip`, so nothing
//! listens beyond this machine. No forward call passes `reverse`; the one reverse rBrowser
//! sets, of a view's loopback port on the device, is `super::reverse`'s (RB-096-12). An adb
//! server someone started with `-a` binds forwards to every interface: [`exposure`] asks the
//! forwarded port at every one of the machine's own network addresses, and a forward that
//! answers there - or one with no address to ask it at - is removed and the view does not
//! open. The socket itself is never handed out: outside clients reach the views only through
//! rBrowser's token-protected CDP proxy (RB-030-06), which does not route Android views.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use serde_json::Value;

/// Chrome's DevTools socket on Android.
pub const CHROME_SOCKET: &str = "localabstract:chrome_devtools_remote";
/// Chrome's package and its main activity.
pub const CHROME_ACTIVITY: &str = "com.android.chrome/com.google.android.apps.chrome.Main";
/// The browser socket's path prefix.
const BROWSER_PATH: &str = "/devtools/browser";

/// `adb devices -l`.
pub fn devices_args() -> Vec<String> {
    strings(&["devices", "-l"])
}

/// `adb -s <serial> forward tcp:0 localabstract:chrome_devtools_remote`: adb prints the port.
pub fn forward_args(serial: &str) -> Vec<String> {
    strings(&["-s", serial, "forward", "tcp:0", CHROME_SOCKET])
}

/// `adb -s <serial> forward --remove tcp:<port>`.
pub fn remove_args(serial: &str, port: u16) -> Vec<String> {
    let local = format!("tcp:{port}");
    strings(&["-s", serial, "forward", "--remove", &local])
}

/// `adb -s <serial> shell wm size`.
pub fn screen_size_args(serial: &str) -> Vec<String> {
    strings(&["-s", serial, "shell", "wm", "size"])
}

/// `adb -s <serial> shell wm density`.
pub fn screen_density_args(serial: &str) -> Vec<String> {
    strings(&["-s", serial, "shell", "wm", "density"])
}

/// `adb -s <serial> shell am start -n <Chrome>`: Chrome opens its DevTools socket once it runs.
pub fn start_chrome_args(serial: &str) -> Vec<String> {
    strings(&["-s", serial, "shell", "am", "start", "-n", CHROME_ACTIVITY])
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

/// The port `adb forward tcp:0` printed; `None` for anything else (an error, port 0).
pub fn parse_port(output: &str) -> Option<u16> {
    output.trim().parse::<u16>().ok().filter(|port| *port != 0)
}

/// The forwarded port on this machine.
pub fn endpoint(port: u16) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port)
}

/// The request for Chrome's `/json/version` over the forwarded port.
pub fn version_request(port: u16) -> String {
    format!("GET /json/version HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n")
}

/// The body of a `200` answer; `None` for another status or no head.
pub fn http_body(answer: &str) -> Option<&str> {
    let (head, body) = answer.split_once("\r\n\r\n")?;
    let status = head.lines().next()?;
    let mut parts = status.split_whitespace();
    (parts.next()?.starts_with("HTTP/") && parts.next()? == "200").then_some(body)
}

/// Chrome's browser socket from its `/json/version` answer, dialled at `127.0.0.1:<port>`
/// whatever host Chrome names; `None` when the answer names none or an odd path.
pub fn browser_socket(port: u16, version: &str) -> Option<String> {
    let answer: Value = serde_json::from_str(version).ok()?;
    let url = answer.get("webSocketDebuggerUrl")?.as_str()?;
    let rest = url.strip_prefix("ws://")?;
    let path = &rest[rest.find('/')?..];
    // Chrome on Android names its discoverable browser target `/devtools/browser`, without the
    // id desktop Chrome adds.
    let plain = match path.strip_prefix(BROWSER_PATH)? {
        "" => true,
        tail => tail.strip_prefix('/').is_some_and(|id| {
            !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        }),
    };
    plain.then(|| format!("ws://127.0.0.1:{port}{path}"))
}

/// Whether `bytes` hold a whole answer: its head and as many body bytes as its
/// `Content-Length` says. Chrome's DevTools server keeps the connection open after answering,
/// so the end of the stream never comes; without a length only that end ends the answer.
pub fn answer_whole(bytes: &[u8]) -> bool {
    let Some(end) = bytes.windows(4).position(|window| window == b"\r\n\r\n") else {
        return false;
    };
    let head = String::from_utf8_lossy(&bytes[..end]);
    head.lines()
        .filter_map(|line| line.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, value)| value.trim().parse::<usize>().ok())
        .is_some_and(|length| bytes.len() >= end + 4 + length)
}

/// Why a forward is not known to answer on loopback only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exposed {
    /// It answers at one of the machine's own network addresses.
    At(SocketAddr),
    /// The machine has no own address to ask it at: nothing proves it is loopback-only.
    Unchecked,
}

/// Whether `port` answers at any of `addresses` beyond loopback (`answers` tries a connection);
/// every address is asked.
///
/// # Errors
///
/// [`Exposed::At`] with the first address it answered at, [`Exposed::Unchecked`] when no
/// address beyond loopback is given.
pub fn exposure(
    port: u16,
    addresses: &[IpAddr],
    answers: impl Fn(SocketAddr) -> bool,
) -> Result<(), Exposed> {
    let mut asked = addresses
        .iter()
        .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
        .map(|ip| SocketAddr::new(*ip, port))
        .peekable();
    if asked.peek().is_none() {
        return Err(Exposed::Unchecked);
    }
    asked
        .find(|addr| answers(*addr))
        .map_or(Ok(()), |addr| Err(Exposed::At(addr)))
}

/// The id of the history entry `step` away from the current one (`-1` back, `1` forward) in
/// `Page.getNavigationHistory`'s answer.
pub fn history_entry(history: &Value, step: i64) -> Option<i64> {
    history_step(history, step)?.get("id")?.as_i64()
}

/// The URL of that entry, so its loopback port can reach this machine first (RB-096-12).
pub fn history_url(history: &Value, step: i64) -> Option<&str> {
    history_step(history, step)?.get("url")?.as_str()
}

fn history_step(history: &Value, step: i64) -> Option<&Value> {
    let current = history.get("currentIndex")?.as_i64()?;
    let index = usize::try_from(current.checked_add(step)?).ok()?;
    history.get("entries")?.get(index)
}

#[cfg(test)]
#[path = "forward_tests.rs"]
mod tests;
