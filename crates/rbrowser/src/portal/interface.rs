//! The address the portal listens on (RB-080-05): `portal.interface` when set, otherwise the
//! local address of the default route (a UDP socket "connected" to a documentation address
//! sends nothing and names it) - but only when that is a private LAN address (RFC 1918,
//! link-local 169.254/16, shared 100.64/10): a public or VPN address there refuses to start and
//! asks the person to set `portal.interface` (Charge 10 review M2). Never every interface:
//! `0.0.0.0` is refused, as are broadcast and multicast addresses; the portal speaks IPv4 only
//! (phase 1).

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};

/// Why there is no address to listen on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum InterfaceError {
    /// `0.0.0.0`: every interface.
    #[error(
        "the portal never listens on every interface (0.0.0.0); set portal.interface to one address"
    )]
    Unspecified,
    /// No IPv4 address of one interface.
    #[error("portal.interface is no IPv4 address of this computer")]
    Invalid,
    /// No default route (no network).
    #[error("no LAN interface found; connect to a network or set portal.interface")]
    NoLan,
    /// The default route's address is no private LAN address (a public one, a VPN's).
    #[error(
        "the default network's address {0} is no private LAN address (it may be public or a \
         VPN's); set portal.interface to the address the portal should listen on"
    )]
    NotPrivate(Ipv4Addr),
}

impl InterfaceError {
    /// The i18n key of the notice that explains it.
    #[must_use]
    pub fn message_key(self) -> &'static str {
        match self {
            Self::NotPrivate(_) => "portal.notice.not_private",
            Self::Unspecified | Self::Invalid | Self::NoLan => "portal.notice.failed",
        }
    }
}

/// The address for `setting` (`portal.interface`; empty: the default route's, if private).
///
/// # Errors
///
/// [`InterfaceError`].
pub fn listen_address(setting: &str) -> Result<Ipv4Addr, InterfaceError> {
    let setting = setting.trim();
    if setting.is_empty() {
        return default_address(default_route());
    }
    let address = setting
        .parse::<Ipv4Addr>()
        .map_err(|_| InterfaceError::Invalid)?;
    if address.is_unspecified() {
        return Err(InterfaceError::Unspecified);
    }
    if address.is_broadcast() || address.is_multicast() {
        return Err(InterfaceError::Invalid);
    }
    Ok(address)
}

/// The default route's address `route`, when it is a private LAN address.
fn default_address(route: Option<Ipv4Addr>) -> Result<Ipv4Addr, InterfaceError> {
    let address = route
        .filter(|address| !address.is_unspecified())
        .ok_or(InterfaceError::NoLan)?;
    if is_private(address) {
        Ok(address)
    } else {
        Err(InterfaceError::NotPrivate(address))
    }
}

/// RFC 1918, link-local (169.254/16) and the shared address space (100.64/10, RFC 6598).
fn is_private(address: Ipv4Addr) -> bool {
    let [first, second, ..] = address.octets();
    address.is_private() || address.is_link_local() || (first == 100 && second & 0xC0 == 64)
}

/// The local IPv4 address of the default route; nothing is sent.
fn default_route() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // TEST-NET-1 (RFC 5737): routed like any outside address, never answered.
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()? {
        SocketAddr::V4(local) => Some(*local.ip()),
        SocketAddr::V6(_) => None,
    }
}

#[cfg(test)]
#[path = "interface_tests.rs"]
mod tests;
