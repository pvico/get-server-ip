//! A utility to find the (dynamic) IPv6 address of a server.
//! Rust port of the original Zig implementation.

mod network;
mod utilities;

use network::local_ethernet;
use utilities::{logger, print};

fn main() {
    let ip = local_ethernet::get_local_ipv6().unwrap_or_else(|err| {
        logger::fatal(&format!("Failed to get IP address: {err}"));
        std::process::exit(1)
    });

    let _ = print::out(&format!("{ip}\n"));
}
