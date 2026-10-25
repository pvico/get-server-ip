//! Local IPv6 address discovery via `getifaddrs`(3).
//! Rust port of the Zig `src/network/local_ethernet.zig` module: enumerates
//! the host's network interfaces and picks the best global IPv6 address.
//!
//! On macOS the address is taken from interface `en0`; on Linux a preferred
//! interface list is probed first (`eth0`, `end0`, `enp0s0`, ...), falling
//! back to the first non-loopback interface with an IPv6 address. Candidates
//! are ranked by scope (link-local < unique-local < global), then by Linux
//! address status (`/proc/net/if_inet6`: dynamic/deprecated/temporary), then
//! by prefix length, then lexicographically.
//!
//! Addresses are represented as `std::net::Ipv6Addr`, while the textual form
//! is produced by the same `inet_ntop`(3) call the Zig implementation used,
//! so the printed output stays byte-compatible.

use std::collections::HashMap;
use std::ffi::CStr;
use std::fmt;
use std::net::Ipv6Addr;
use std::os::raw::{c_char, c_void};
use std::ptr;

use libc::{ifaddrs, sockaddr, sockaddr_in6};

use crate::utilities::{logger, print};

/// Errors that can occur while looking up the local IPv6 address.
/// The `Display` output keeps the Zig error names so log output matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GetIpError {
    /// `getifaddrs`(3) failed.
    GetIfAddrsFailed,
    /// `inet_ntop`(3) could not format the address.
    FormatIPv6Failed,
    /// No usable IPv6 address was found on the selected interface.
    Ipv6NotFound,
}

impl fmt::Display for GetIpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            GetIpError::GetIfAddrsFailed => "GetIfAddrsFailed",
            GetIpError::FormatIPv6Failed => "FormatIPv6Failed",
            GetIpError::Ipv6NotFound => "IPv6NotFound",
        };
        f.write_str(name)
    }
}

impl std::error::Error for GetIpError {}

/// How "good" an IPv6 address is for external use; higher is better.
/// Mirrors the Zig `AddressScope` enum and its discriminant values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AddressScope {
    Other = 0,
    LinkLocal = 1,
    UniqueLocal = 2,
    Global = 3,
}

/// Extra status flags attached to an IPv6 address by the Linux kernel
/// (`/proc/net/if_inet6`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AddressStatus {
    pub dynamic: bool,
    pub deprecated: bool,
    pub temporary: bool,
}

impl AddressStatus {
    /// Preference ranking: lower is better. Mirrors the Zig `rank`:
    /// bit 0 is `temporary`, bit 1 is `deprecated`.
    pub fn rank(self) -> u8 {
        let mut value: u8 = 0;
        if self.temporary {
            value |= 1;
        }
        if self.deprecated {
            value |= 2;
        }
        value
    }

    /// Human-readable status: "stable" or the flags joined with '+'.
    pub fn format(self) -> String {
        if !self.dynamic && !self.deprecated && !self.temporary {
            return "stable".to_string();
        }
        let mut parts: Vec<&str> = Vec::new();
        if self.dynamic {
            parts.push("dynamic");
        }
        if self.temporary {
            parts.push("temporary");
        }
        if self.deprecated {
            parts.push("deprecated");
        }
        parts.join("+")
    }
}

/// Build `AddressStatus` from the Linux `IFA_F_*` flag bits:
/// `0x80` is `IFA_F_PERMANENT`, `0x01` is `IFA_F_TEMPORARY`,
/// `0x20` is `IFA_F_DEPRECATED`.
fn address_status_from_flags(flags: u32) -> AddressStatus {
    AddressStatus {
        dynamic: (flags & 0x80) == 0,
        temporary: (flags & 0x01) != 0,
        deprecated: (flags & 0x20) != 0,
    }
}

/// Classify an IPv6 address by scope. Returns `None` for addresses that can
/// never be used externally (unspecified, loopback, multicast).
pub fn classify_ipv6(addr: &Ipv6Addr) -> Option<AddressScope> {
    let octets = addr.octets();

    if octets == [0u8; 16] {
        return None; // unspecified ::
    }

    let mut loopback = true;
    for &b in &octets[..15] {
        if b != 0 {
            loopback = false;
            break;
        }
    }
    if loopback && octets[15] == 1 {
        return None; // ::1
    }

    if octets[0] == 0xff {
        return None; // multicast
    }

    if octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80 {
        return Some(AddressScope::LinkLocal); // fe80::/10
    }

    if (octets[0] & 0xfe) == 0xfc {
        return Some(AddressScope::UniqueLocal); // fc00::/7
    }

    if (octets[0] & 0xe0) == 0x20 {
        return Some(AddressScope::Global); // 2000::/3
    }

    Some(AddressScope::Other)
}

/// Count the leading one bits of an address, i.e. the prefix length it
/// encodes when it is a netmask.
pub fn ipv6_prefix_length(addr: &Ipv6Addr) -> u8 {
    let octets = addr.octets();
    let mut total: u8 = 0;
    for &b in octets.iter() {
        if b == 0xff {
            total += 8;
            continue;
        }

        let mut value = b;
        while value & 0x80 != 0 {
            total += 1;
            value <<= 1;
        }
        break;
    }
    total
}

/// Extract the prefix length from the `ifa_netmask` sockaddr returned by
/// `getifaddrs`(3). Non-IPv6 or missing netmasks count as a /128.
fn prefix_length_from_netmask(netmask: *const sockaddr) -> u8 {
    if netmask.is_null() {
        return 128;
    }
    unsafe {
        if (*netmask).sa_family as libc::c_int != libc::AF_INET6 {
            return 128;
        }

        let sa6 = netmask as *const sockaddr_in6;
        let addr = Ipv6Addr::from((*sa6).sin6_addr.s6_addr);
        ipv6_prefix_length(&addr)
    }
}

// The `libc` crate does not export `inet_ntop` on every platform (e.g.
// macOS), so it is declared here against the system libc.
unsafe extern "C" {
    fn inet_ntop(
        af: libc::c_int,
        src: *const c_void,
        dst: *mut c_char,
        size: libc::socklen_t,
    ) -> *const c_char;
}

/// Per RFC 2553: buffer size for the longest IPv6 text form.
const INET6_ADDRSTRLEN: usize = 46;

/// Format 16 address bytes with `inet_ntop`(3), exactly as the Zig
/// implementation did, for byte-compatible output.
fn format_ipv6(octets: [u8; 16]) -> Result<String, GetIpError> {
    let mut buf = [0u8; INET6_ADDRSTRLEN];
    let formatted = unsafe {
        inet_ntop(
            libc::AF_INET6,
            octets.as_ptr() as *const c_void,
            buf.as_mut_ptr() as *mut c_char,
            INET6_ADDRSTRLEN as libc::socklen_t,
        )
    };
    if formatted.is_null() {
        return Err(GetIpError::FormatIPv6Failed);
    }
    let cstr = unsafe { CStr::from_ptr(buf.as_ptr() as *const c_char) };
    Ok(cstr.to_string_lossy().into_owned())
}

/// True when an interface with the given name is present in the `getifaddrs`
/// list.
fn interface_exists(list: *const ifaddrs, name: &[u8]) -> bool {
    let mut cursor = list;
    while !cursor.is_null() {
        let node = unsafe { &*cursor };
        if unsafe { CStr::from_ptr(node.ifa_name) }.to_bytes() == name {
            return true;
        }
        cursor = node.ifa_next;
    }
    false
}

/// Read `/proc/net/if_inet6` and map every address of `interface_name` to
/// its kernel status flags. Non-Linux platforms get an empty map.
fn linux_load_address_status(interface_name: &[u8]) -> HashMap<[u8; 16], AddressStatus> {
    let mut map: HashMap<[u8; 16], AddressStatus> = HashMap::new();

    if !cfg!(target_os = "linux") {
        return map;
    }

    let file_data = match std::fs::read_to_string("/proc/net/if_inet6") {
        Ok(data) => data,
        Err(err) => {
            logger::warn(&format!("Unable to read /proc/net/if_inet6: {err}"));
            return map;
        }
    };

    for line in file_data.lines() {
        if line.is_empty() {
            continue;
        }

        // Columns: address(32 hex) ifindex prefixlen scopeid flags ifname.
        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 6 {
            continue;
        }
        let addr_token = tokens[0];
        let flags_token = tokens[4];
        let name_token = tokens[5];

        if name_token.as_bytes() != interface_name {
            continue;
        }
        if addr_token.len() != 32 {
            continue;
        }

        let mut addr_bytes: [u8; 16] = [0; 16];
        let mut valid = true;
        for i in (0..32).step_by(2) {
            match u8::from_str_radix(&addr_token[i..i + 2], 16) {
                Ok(value) => addr_bytes[i / 2] = value,
                Err(_) => {
                    valid = false;
                    break;
                }
            }
        }
        if !valid {
            continue;
        }

        let flags = match u32::from_str_radix(flags_token, 16) {
            Ok(flags) => flags,
            Err(_) => continue,
        };
        map.insert(addr_bytes, address_status_from_flags(flags));
    }

    map
}

/// Debug helper (parity with the Zig `debugPrintLinuxInterfaces`): print
/// every `/proc/net/if_inet6` line with its decoded status to stderr.
#[allow(dead_code)] // parity with the Zig utility API
pub fn debug_print_linux_interfaces() {
    if !cfg!(target_os = "linux") {
        return;
    }

    let file_data = match std::fs::read_to_string("/proc/net/if_inet6") {
        Ok(data) => data,
        Err(err) => {
            logger::warn(&format!("Unable to read /proc/net/if_inet6: {err}"));
            return;
        }
    };

    for line in file_data.lines() {
        if line.is_empty() {
            continue;
        }

        let tokens: Vec<&str> = line.split_whitespace().collect();
        if tokens.len() < 6 {
            continue;
        }
        let flags = match u32::from_str_radix(tokens[4], 16) {
            Ok(flags) => flags,
            Err(_) => continue,
        };
        let status = address_status_from_flags(flags);

        let _ = print::err(&format!("{line} {}\n", status.format()));
    }
}

/// Best candidate found while scanning the interface list.
struct BestAddress {
    address: String,
    scope: AddressScope,
    prefix: u8,
    status: AddressStatus,
}

/// Decide whether a candidate address is preferable to the current best.
/// Mirrors the Zig `shouldPrefer`: scope first (non-deprecated wins on an
/// upgrade), then status rank, then longer prefix, then lexicographic order.
#[allow(clippy::too_many_arguments)] // faithful port of the Zig signature
fn should_prefer(
    new_scope: AddressScope,
    new_prefix: u8,
    new_addr: &str,
    current_scope: AddressScope,
    current_prefix: u8,
    current_addr: &str,
    new_status: AddressStatus,
    current_status: AddressStatus,
) -> bool {
    if new_scope != current_scope {
        if new_scope > current_scope {
            return !(new_status.deprecated && !current_status.deprecated);
        }

        if !new_status.deprecated && current_status.deprecated {
            return true;
        }

        return false;
    }

    let new_rank = new_status.rank();
    let current_rank = current_status.rank();

    if new_rank != current_rank {
        return new_rank < current_rank;
    }

    if new_prefix != current_prefix {
        return new_prefix > current_prefix;
    }

    new_addr < current_addr
}

/// Interfaces probed in order on Linux before falling back to the first
/// non-loopback interface with an IPv6 address.
const PREFERRED_INTERFACES: &[&[u8]] = &[
    b"eth0", b"end0", b"enp0s0", b"enp1s0", b"eno1", b"ens33", b"en0", b"wlan0",
];

/// Walk the `getifaddrs` list and pick the best IPv6 address, as the Zig
/// `getLocalIPv6` did. The list must be freed by the caller.
fn find_best_ipv6(list: *const ifaddrs) -> Result<String, GetIpError> {
    let mut interface_name: &[u8] = if cfg!(target_os = "macos") {
        b"en0"
    } else {
        b"eth0"
    };

    if cfg!(target_os = "linux") {
        let preferred = PREFERRED_INTERFACES
            .iter()
            .find(|candidate| interface_exists(list, candidate));
        interface_name = match preferred {
            Some(candidate) => candidate,
            None => {
                let mut fallback: Option<&[u8]> = None;
                let mut cursor = list;
                while !cursor.is_null() {
                    let node = unsafe { &*cursor };
                    cursor = node.ifa_next;

                    let has_ipv6 = !node.ifa_addr.is_null()
                        && unsafe { (*node.ifa_addr).sa_family as libc::c_int } == libc::AF_INET6;
                    if has_ipv6 && node.ifa_flags & libc::IFF_LOOPBACK as u32 == 0 {
                        fallback = Some(unsafe { CStr::from_ptr(node.ifa_name) }.to_bytes());
                        break;
                    }
                }
                fallback.unwrap_or(interface_name)
            }
        };
    }

    let linux_status_map = linux_load_address_status(interface_name);

    let mut best: Option<BestAddress> = None;
    let mut cursor = list;
    while !cursor.is_null() {
        let node = unsafe { &*cursor };
        cursor = node.ifa_next;

        if unsafe { CStr::from_ptr(node.ifa_name) }.to_bytes() != interface_name {
            continue;
        }
        if node.ifa_addr.is_null() {
            continue;
        }
        if unsafe { (*node.ifa_addr).sa_family as libc::c_int } != libc::AF_INET6 {
            continue;
        }

        let sa6 = node.ifa_addr as *const sockaddr_in6;
        let octets = unsafe { (*sa6).sin6_addr.s6_addr };
        let addr = Ipv6Addr::from(octets);
        let addr_string = format_ipv6(octets)?;

        let scope = match classify_ipv6(&addr) {
            Some(scope) => scope,
            None => continue,
        };

        let mut status = AddressStatus::default();
        if let Some(entry) = linux_status_map.get(&octets) {
            status = *entry;
        }

        if status.temporary {
            continue;
        }

        let prefix_len = prefix_length_from_netmask(node.ifa_netmask);

        let prefer = match &best {
            Some(current) => should_prefer(
                scope,
                prefix_len,
                &addr_string,
                current.scope,
                current.prefix,
                &current.address,
                status,
                current.status,
            ),
            None => true,
        };
        if prefer {
            best = Some(BestAddress {
                address: addr_string,
                scope,
                prefix: prefix_len,
                status,
            });
        }
    }

    best.map(|best| best.address)
        .ok_or(GetIpError::Ipv6NotFound)
}

/// Return the best local IPv6 address as text (e.g. `fe80::1`).
pub fn get_local_ipv6() -> Result<String, GetIpError> {
    let mut list: *mut ifaddrs = ptr::null_mut();
    if unsafe { libc::getifaddrs(&mut list) } != 0 {
        return Err(GetIpError::GetIfAddrsFailed);
    }

    let result = find_best_ipv6(list);

    unsafe { libc::freeifaddrs(list) };

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(s: &str) -> Option<AddressScope> {
        classify_ipv6(&s.parse().unwrap())
    }

    #[test]
    fn classify_rejects_unusable_addresses() {
        assert_eq!(classify("::"), None);
        assert_eq!(classify("::1"), None);
        assert_eq!(classify("ff02::1"), None);
    }

    #[test]
    fn classify_scopes() {
        assert_eq!(classify("fe80::1"), Some(AddressScope::LinkLocal));
        // febf::1 is in the upper half of fe80::/10.
        assert_eq!(classify("febf::1"), Some(AddressScope::LinkLocal));
        // fec0::/10 (old site-local) is not in fe80::/10: falls through to Other.
        assert_eq!(classify("fec0::1"), Some(AddressScope::Other));
        assert_eq!(classify("fc00::1"), Some(AddressScope::UniqueLocal));
        assert_eq!(classify("fd12:3456::1"), Some(AddressScope::UniqueLocal));
        assert_eq!(classify("2001:db8::1"), Some(AddressScope::Global));
        assert_eq!(classify("2002::1"), Some(AddressScope::Global));
        assert_eq!(classify("::ffff:192.0.2.1"), Some(AddressScope::Other));
    }

    #[test]
    fn prefix_lengths() {
        let cases: &[(&str, u8)] = &[
            ("::", 0),
            ("8000::", 1),
            ("ffff:ff00::", 24),
            ("ffff:ffff::", 32),
            ("ffff:ffff:ffff:ffff::", 64),
            ("ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff", 128),
        ];
        for (addr, expected) in cases {
            let addr: Ipv6Addr = addr.parse().unwrap();
            assert_eq!(ipv6_prefix_length(&addr), *expected, "prefix of {addr}");
        }
    }

    #[test]
    fn netmask_prefix_lengths() {
        assert_eq!(prefix_length_from_netmask(ptr::null()), 128);

        // Same sockaddr_in6 union reused for both sub-cases.
        let mut sa: sockaddr_in6 = unsafe { std::mem::zeroed() };
        sa.sin6_family = libc::AF_INET6 as _;
        sa.sin6_addr.s6_addr = [0xff; 16];
        sa.sin6_addr.s6_addr[4..].fill(0);
        assert_eq!(
            prefix_length_from_netmask(&sa as *const sockaddr_in6 as *const sockaddr),
            32
        );

        // Wrong address family counts as /128.
        let mut v4: sockaddr_in6 = unsafe { std::mem::zeroed() };
        v4.sin6_family = libc::AF_INET as _;
        assert_eq!(
            prefix_length_from_netmask(&v4 as *const sockaddr_in6 as *const sockaddr),
            128
        );
    }

    #[test]
    fn formats_like_inet_ntop() {
        assert_eq!(format_ipv6([0u8; 16]).unwrap(), "::");
        assert_eq!(
            format_ipv6([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap(),
            "::1"
        );
        assert_eq!(
            format_ipv6([0xfe, 0x80, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]).unwrap(),
            "fe80::1"
        );
        assert_eq!(
            format_ipv6([
                0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x01
            ])
            .unwrap(),
            "2001:db8::1"
        );
        assert_eq!(
            format_ipv6([0xff; 16]).unwrap(),
            "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"
        );
    }

    #[test]
    fn status_ranks() {
        assert_eq!(AddressStatus::default().rank(), 0);
        assert_eq!(
            AddressStatus {
                dynamic: true,
                ..Default::default()
            }
            .rank(),
            0
        );
        assert_eq!(
            AddressStatus {
                temporary: true,
                ..Default::default()
            }
            .rank(),
            1
        );
        assert_eq!(
            AddressStatus {
                deprecated: true,
                ..Default::default()
            }
            .rank(),
            2
        );
        assert_eq!(
            AddressStatus {
                temporary: true,
                deprecated: true,
                ..Default::default()
            }
            .rank(),
            3
        );
    }

    #[test]
    fn status_formatting() {
        assert_eq!(AddressStatus::default().format(), "stable");
        assert_eq!(
            AddressStatus {
                dynamic: true,
                ..Default::default()
            }
            .format(),
            "dynamic"
        );
        assert_eq!(
            AddressStatus {
                dynamic: true,
                temporary: true,
                ..Default::default()
            }
            .format(),
            "dynamic+temporary"
        );
        assert_eq!(
            AddressStatus {
                dynamic: true,
                deprecated: true,
                ..Default::default()
            }
            .format(),
            "dynamic+deprecated"
        );
        assert_eq!(
            AddressStatus {
                dynamic: true,
                temporary: true,
                deprecated: true,
            }
            .format(),
            "dynamic+temporary+deprecated"
        );
    }

    #[test]
    fn status_from_flags() {
        // Permanent address.
        assert_eq!(
            address_status_from_flags(0x80),
            AddressStatus {
                dynamic: false,
                deprecated: false,
                temporary: false
            }
        );
        // Autoconf address without extra flags.
        assert_eq!(
            address_status_from_flags(0x00),
            AddressStatus {
                dynamic: true,
                deprecated: false,
                temporary: false
            }
        );
        // Temporary and deprecated bits.
        assert_eq!(
            address_status_from_flags(0x21),
            AddressStatus {
                dynamic: true,
                deprecated: true,
                temporary: true
            }
        );
    }

    #[test]
    fn scope_beats_everything() {
        let stable = AddressStatus::default();
        assert!(should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::1",
            AddressScope::LinkLocal,
            128,
            "fe80::1",
            stable,
            stable
        ));
        // ...unless the better-scoped address is deprecated.
        assert!(!should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::1",
            AddressScope::LinkLocal,
            128,
            "fe80::1",
            AddressStatus {
                deprecated: true,
                ..Default::default()
            },
            stable
        ));
        // A non-deprecated address can upgrade over a deprecated one.
        assert!(should_prefer(
            AddressScope::LinkLocal,
            64,
            "fe80::2",
            AddressScope::Global,
            64,
            "2001:db8::1",
            stable,
            AddressStatus {
                deprecated: true,
                ..Default::default()
            }
        ));
        assert!(!should_prefer(
            AddressScope::LinkLocal,
            64,
            "fe80::2",
            AddressScope::Global,
            64,
            "2001:db8::1",
            stable,
            stable
        ));
    }

    #[test]
    fn status_rank_tiebreak() {
        let temporary = AddressStatus {
            temporary: true,
            ..Default::default()
        };
        assert!(should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::2",
            AddressScope::Global,
            64,
            "2001:db8::1",
            AddressStatus::default(),
            temporary
        ));
        assert!(!should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::2",
            AddressScope::Global,
            64,
            "2001:db8::1",
            temporary,
            AddressStatus::default()
        ));
    }

    #[test]
    fn prefix_then_lexical_tiebreak() {
        let stable = AddressStatus::default();
        assert!(should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::2",
            AddressScope::Global,
            32,
            "2001:db8::1",
            stable,
            stable
        ));
        assert!(should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::a",
            AddressScope::Global,
            64,
            "2001:db8::b",
            stable,
            stable
        ));
        // Fully equal candidates never replace the current best.
        assert!(!should_prefer(
            AddressScope::Global,
            64,
            "2001:db8::1",
            AddressScope::Global,
            64,
            "2001:db8::1",
            stable,
            stable
        ));
    }
}
