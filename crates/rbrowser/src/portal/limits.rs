//! How many connections the portal serves at once (RB-080-05, Charge 10 review L2): at most
//! [`MAX_CONNECTIONS`] in all and [`MAX_PER_ADDRESS`] from one address, so one device - or one
//! machine in the LAN that never paired - cannot hold every slot. More are closed at once.

use std::collections::HashMap;
use std::net::IpAddr;

/// Connections served at once.
pub const MAX_CONNECTIONS: usize = 64;
/// Connections served at once from one address.
pub const MAX_PER_ADDRESS: usize = 16;

/// The connections being served, in all and per address.
#[derive(Debug, Default)]
pub struct Limits {
    total: usize,
    per_address: HashMap<IpAddr, usize>,
    /// Every connection comes from one address (the presentation's tunnel, RB-096-05): the
    /// limit per address is the limit in all.
    shared_address: bool,
}

impl Limits {
    /// The limits of a listener whose connections all come from one address.
    #[must_use]
    pub fn shared_address() -> Self {
        Self {
            shared_address: true,
            ..Self::default()
        }
    }

    /// Takes a slot for a connection from `ip`; `false` when it must be closed.
    pub fn acquire(&mut self, ip: IpAddr) -> bool {
        if self.total >= MAX_CONNECTIONS {
            return false;
        }
        let cap = if self.shared_address {
            MAX_CONNECTIONS
        } else {
            MAX_PER_ADDRESS
        };
        let open = self.per_address.entry(ip).or_default();
        if *open >= cap {
            return false;
        }
        *open += 1;
        self.total += 1;
        true
    }

    /// Gives back a slot of `ip`.
    pub fn release(&mut self, ip: IpAddr) {
        if let Some(open) = self.per_address.get_mut(&ip) {
            *open = open.saturating_sub(1);
            if *open == 0 {
                self.per_address.remove(&ip);
            }
            self.total = self.total.saturating_sub(1);
        }
    }

    /// The connections served now.
    #[must_use]
    pub fn total(&self) -> usize {
        self.total
    }

    /// The addresses with a connection now.
    #[must_use]
    pub fn addresses(&self) -> usize {
        self.per_address.len()
    }
}

#[cfg(test)]
#[path = "limits_tests.rs"]
mod tests;
