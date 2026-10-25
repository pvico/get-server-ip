//! Hostname resolution for the DDNS sources.
//! Rust port of the Zig `src/dns/dns.zig_` module. The Zig `getIpV6`
//! variant allocated its buffer but never actually formatted the address;
//! this port completes it. Addresses are returned in canonical RFC 5952
//! form (`Ipv6Addr` Display) so they compare equal regardless of how the
//! source spelled them.

use std::io;
use std::net::ToSocketAddrs;

/// Errors returned by the resolvers (names keep the Zig error naming).
#[derive(Debug)]
pub enum ResolveError {
    /// The system resolver failed.
    Io(io::Error),
    /// The resolver returned no address of the requested family.
    NoAddressFound,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolveError::Io(err) => write!(f, "{err}"),
            ResolveError::NoAddressFound => f.write_str("NoAddressFound"),
        }
    }
}

impl std::error::Error for ResolveError {}

impl From<io::Error> for ResolveError {
    fn from(err: io::Error) -> Self {
        ResolveError::Io(err)
    }
}

/// Resolves `domain` and returns its first IPv6 address (Zig `getIpV6`).
pub fn resolve_ipv6(domain: &str) -> Result<String, ResolveError> {
    first_address(domain, false)
}

/// Resolves `domain` and returns its first IPv4 address as dotted text
/// (Zig `getIp`).
#[allow(dead_code)] // parity with the Zig getIp variant
pub fn resolve_ipv4(domain: &str) -> Result<String, ResolveError> {
    first_address(domain, true)
}

/// Resolve via the system resolver (port 0: the port is irrelevant for
/// lookup) and return the first address of the requested family.
fn first_address(domain: &str, ipv4: bool) -> Result<String, ResolveError> {
    let addrs = (domain, 0u16).to_socket_addrs()?;
    for addr in addrs {
        if addr.is_ipv4() == ipv4 {
            return Ok(addr.ip().to_string());
        }
    }
    Err(ResolveError::NoAddressFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_localhost() {
        // /etc/hosts lists both families for localhost.
        assert_eq!(resolve_ipv4("localhost").unwrap(), "127.0.0.1");
        assert_eq!(resolve_ipv6("localhost").unwrap(), "::1");
    }

    #[test]
    fn no_address_of_family() {
        // A v4 literal has no IPv6 address to return.
        assert!(matches!(
            resolve_ipv6("0.0.0.0"),
            Err(ResolveError::NoAddressFound)
        ));
        // ...and vice versa.
        assert!(matches!(
            resolve_ipv4("::1"),
            Err(ResolveError::NoAddressFound)
        ));
    }

    #[test]
    fn resolver_failure_is_reported() {
        // The trailing dot prevents search-domain appending; `.invalid` is
        // guaranteed never to resolve.
        assert!(resolve_ipv6("nonexistent.get-server-ip.invalid.").is_err());
        assert!(resolve_ipv6("").is_err());
    }
}
