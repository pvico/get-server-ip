# get-server-ip

![Rust version](https://img.shields.io/badge/Rust_version-edition_2024-green)

A utility to find the (dynamic) IPv6 address of a server. Initially developed
in Zig, now implemented in Rust.

It will

- check the GitHub gist file containing the IP
- check all the dynamic DNS services addresses pointing to this IP
- do that concurrently and with a time bound duration
- return the IP if at least the GitHub gist file IP and one DDNS service IP
  are the same
- return the IP matching at least two of the DDNS services if there is no
  match between the GitHub gist file IP and one DDNS service IP (e.g. if the
  GitHub gist file IP is not available)
- fall back to the best local interface IP when no source is available

These IP sources are updated by some external automation (e.g. n8n workflow,
node-red, etc.).

## Building

Requires a Rust toolchain (edition 2024, e.g. installed via rustup).

```bash
cargo build --release
```

The release profile is size-optimized: the utility is invoked per SSH
connection, so a small binary keeps process startup cheap.

### Building on the target devices

The servers run Linux (aarch64 or x86_64) and have rustup installed, so
release builds happen natively on each device over SSH — no cross-compilation
and no Zig involved:

```bash
./scripts/build-remote.sh rpi5-server nanopi-r3s-lts btcnode
```

The binaries land in `dist/<host>/get-server-ip`.

## Configuration

Configure the gist file URI and the DDNS service domains in the `config.json`
file. This file must be located in a subdirectory `.get-server-ip` of the CLI
executable location, or the path can be given explicitly with
`--config FILE_PATH`. The schema matches the checked-in `config.json`
template:

```json
{
    "gist-uri": "https://gist.github.com/<my user name>/1234567890abcdef",
    "ddns-domains": [
        "mysubdomain.example.com",
        "mysubdomain.example.org",
        "mysubdomain.example.net"
    ]
}
```

## Usage

```bash
./get-server-ip
```

```text
Usage: get-server-ip [OPTIONS]

Options:
  -c, --config <FILE_PATH>  Path to the configuration file (default: .get-server-ip/config.json next to the executable)
  -v, --verbose...          Increase the verbosity of the log output (-v: info, -vv: debug)
  -h, --help                Print help
  -V, --version             Print version
```

All log output goes to stderr (warnings and errors always, info with `-v`,
debug with `-vv` or in debug builds); stdout only ever carries the IP, so the
output can be used directly in command substitution:

```bash
ssh -p 2222 -o UserKnownHostsFile=/dev/null -o StrictHostKeyChecking=no "$(get-server-ip 2>/dev/null)"
```

## Development

```bash
cargo test
cargo clippy --all-targets
cargo fmt --check
```
