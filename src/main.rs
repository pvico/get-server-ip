//! A utility to find the (dynamic) IPv6 address of a server.
//! Rust port of the original Zig implementation.

mod cli_interface;
mod config;
mod network;
mod utilities;

use utilities::{logger, print};

fn main() {
    let cli = cli_interface::parse();
    // Debug builds start fully verbose, preserving the Zig DEBUG_MODE
    // behavior during development; release builds need -v/-vv.
    logger::set_verbosity(cli.verbose.max(if cfg!(debug_assertions) { 2 } else { 0 }));

    let _config = config::init_from_json_file(cli.config.as_deref());

    let ip = network::local_ethernet::get_local_ipv6().unwrap_or_else(|err| {
        logger::fatal(&format!("Failed to get IP address: {err}"));
        std::process::exit(1)
    });

    let _ = print::out(&format!("{ip}\n"));
}
