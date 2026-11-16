//! The network rule of the plugins' `http` capability (Charge 6; review finding F5 of Charge 5):
//! a plugin reaches public addresses only - never loopback, private, link-local, shared,
//! multicast, documentation or other special-purpose ranges, IPv4 inside IPv6 included (mapped,
//! translated, NAT64, 6to4, Teredo) - unless
//! the URL's origin is one of the open project's own (its `startUrl` and environments), so a
//! development server on `localhost` stays reachable for the project's own checks. The rule is
//! applied to the addresses a name resolves to, and the request connects only to those checked
//! ([`super::http`]), so neither an internal name nor a rebinding DNS answer gets through.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ureq::http::Uri;

/// Scheme, host and port of a URL: what "the same origin" compares.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Origin {
    scheme: String,
    host: String,
    port: u16,
}

impl Origin {
    /// The origin of an absolute `http`/`https` URL; `None` for anything else.
    pub fn of(url: &str) -> Option<Self> {
        Self::of_uri(&url.parse::<Uri>().ok()?)
    }

    /// The origin of `uri`, as [`Origin::of`].
    pub fn of_uri(uri: &Uri) -> Option<Self> {
        let scheme = uri.scheme_str()?.to_ascii_lowercase();
        let default = match scheme.as_str() {
            "http" => 80,
            "https" => 443,
            _ => return None,
        };
        let host = uri.host()?.to_ascii_lowercase();
        if host.is_empty() {
            return None;
        }
        Some(Self {
            scheme,
            host,
            port: uri.port_u16().unwrap_or(default),
        })
    }
}

/// Whether `ip` is a public unicast address.
pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => is_public_v4(ip),
        IpAddr::V6(ip) => is_public_v6(ip),
    }
}

fn is_public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    let special = a == 0
        || a == 10
        || a == 127
        || a >= 224
        || (a == 100 && (64..128).contains(&b))
        || (a == 169 && b == 254)
        || (a == 172 && (16..32).contains(&b))
        || (a == 192 && b == 168)
        || (a == 192 && b == 0 && c == 0)
        || (a == 192 && b == 0 && c == 2)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113);
    !special
}

fn is_public_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    // IPv4-mapped (::ffff:a.b.c.d), IPv4-translated (::ffff:0:a.b.c.d) and NAT64
    // (64:ff9b::/96): the IPv4 address inside decides.
    let embedded = |high: [u16; 6]| -> Option<Ipv4Addr> {
        (segments[..6] == high).then(|| {
            let [a, b] = segments[6].to_be_bytes();
            let [c, d] = segments[7].to_be_bytes();
            Ipv4Addr::new(a, b, c, d)
        })
    };
    if let Some(v4) = embedded([0, 0, 0, 0, 0, 0xffff])
        .or_else(|| embedded([0, 0, 0, 0, 0xffff, 0]))
        .or_else(|| embedded([0x64, 0xff9b, 0, 0, 0, 0]))
    {
        return is_public_v4(v4);
    }
    if segments[..6] == [0, 0, 0, 0, 0, 0] {
        // `::`, `::1` and the deprecated IPv4-compatible range.
        return false;
    }
    let [first, second, third, fourth, ..] = segments;
    let special = (first & 0xfe00) == 0xfc00 // unique local fc00::/7
        || (first & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (first & 0xffc0) == 0xfec0 // site-local fec0::/10
        || (first & 0xff00) == 0xff00 // multicast
        || (first == 0x2001 && second == 0x0db8) // documentation
        // Review finding L3 of Charge 6: ranges that lead to private or special hosts.
        || (first == 0x0064 && second == 0xff9b && third == 0x0001) // local NAT64 64:ff9b:1::/48
        || (first == 0x0100 && second == 0 && third == 0 && fourth == 0) // discard 100::/64
        || (first == 0x2001 && second < 0x0200) // IETF protocols 2001::/23 (Teredo, ORCHID)
        || first == 0x2002 // 6to4 2002::/16, an IPv4 address inside
        || (first == 0x3fff && second < 0x1000) // documentation 3fff::/20
        || first == 0x5f00; // SRv6 SIDs 5f00::/16
    !special
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
