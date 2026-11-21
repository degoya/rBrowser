//! The way from the device back to this machine (RB-096-12, owner's decision 2026-11-21),
//! pure: on the device, `localhost` is the device itself, so a view that loads a loopback
//! address ([`loopback_port`]) gets `adb reverse` for exactly that port ([`reverse_args`]).
//!
//! **Security (the narrower rule that replaces RB-095-02's "no call passes reverse"):** a
//! reverse names `tcp:<port>` of the same port on both sides - never a range, an address, `-a`,
//! `tcpip` or `--remove-all` - and is added with `--no-rebind`, so it never takes over a reverse
//! someone else set. The list ([`parse_reverse_list`]) is read first; a port that is listed and
//! not rBrowser's own is foreign ([`decide`]) and neither replaced nor removed. Only the hosts
//! `localhost`, `127.0.0.1` and `[::1]` count; while a reverse stands, any app on the device can
//! reach that one port (the setting `engines.android.reverseLocalhost` says so).

use std::net::{Ipv4Addr, Ipv6Addr};

use url::{Host, Url};

/// `adb -s <serial> reverse --no-rebind tcp:<port> tcp:<port>`.
pub fn reverse_args(serial: &str, port: u16) -> Vec<String> {
    let spec = tcp(port);
    strings(&["-s", serial, "reverse", "--no-rebind", &spec, &spec])
}

/// `adb -s <serial> reverse --remove tcp:<port>`.
pub fn unreverse_args(serial: &str, port: u16) -> Vec<String> {
    strings(&["-s", serial, "reverse", "--remove", &tcp(port)])
}

/// `adb -s <serial> reverse --list`.
pub fn reverse_list_args(serial: &str) -> Vec<String> {
    strings(&["-s", serial, "reverse", "--list"])
}

fn tcp(port: u16) -> String {
    format!("tcp:{port}")
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_owned()).collect()
}

/// The loopback URL's host and port: `http` or `https` on `localhost`, `127.0.0.1` or
/// `[::1]`, its port or the scheme's (80, 443).
fn loopback(url: &str) -> Option<(Url, u16)> {
    let parsed = Url::parse(url).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let loopback = match parsed.host()? {
        Host::Domain(name) => name.eq_ignore_ascii_case("localhost"),
        Host::Ipv4(ip) => ip == Ipv4Addr::LOCALHOST,
        Host::Ipv6(ip) => ip == Ipv6Addr::LOCALHOST,
    };
    let port = parsed.port_or_known_default().filter(|port| *port != 0)?;
    loopback.then_some((parsed, port))
}

/// The port of this machine a view loading `url` needs on the device; `None` for another
/// host, another scheme or no URL.
pub fn loopback_port(url: &str) -> Option<u16> {
    loopback(url).map(|(_, port)| port)
}

/// `host:port` of a loopback `url`, as a notice names it.
pub fn address_of(url: &str) -> Option<String> {
    let (parsed, port) = loopback(url)?;
    Some(format!("{}:{port}", parsed.host_str()?))
}

/// One line of `adb reverse --list`: the device's side and this machine's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    /// On the device (`tcp:3000`).
    pub device: String,
    /// On this machine (`tcp:3000`).
    pub host: String,
}

impl Listed {
    /// A listed reverse.
    pub fn new(device: &str, host: &str) -> Self {
        Self {
            device: device.to_owned(),
            host: host.to_owned(),
        }
    }
}

/// The reverses `adb reverse --list` printed: lines of `[<transport>] <device> <host>`; any
/// other line is skipped.
pub fn parse_reverse_list(output: &str) -> Vec<Listed> {
    output
        .lines()
        .filter_map(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            match words.as_slice() {
                [device, host] | [_, device, host]
                    if device.contains(':') && host.contains(':') =>
                {
                    Some(Listed::new(device, host))
                }
                _ => None,
            }
        })
        .collect()
}

/// What to do for a port.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// rBrowser's own reverse stands.
    Stands,
    /// Someone else's reverse holds the port on the device: left alone.
    Foreign,
    /// Nothing holds it: set it.
    Set,
}

/// What to do for `port`, given the `listed` reverses and whether rBrowser set it (`own`).
pub fn decide(port: u16, listed: &[Listed], own: bool) -> Decision {
    let spec = tcp(port);
    match listed.iter().find(|entry| entry.device == spec) {
        None => Decision::Set,
        Some(entry) if own && entry.host == spec => Decision::Stands,
        Some(_) => Decision::Foreign,
    }
}

#[cfg(test)]
#[path = "reverse_tests.rs"]
mod tests;
