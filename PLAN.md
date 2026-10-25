# Plan: Convert get-server-ip from Zig to Rust

Port the `get-server-ip` utility from its Zig implementation
(`~/Development/zig/get-server-ip`, GitHub `pvico/get-server-ip`, branch
`master`) to Rust, following the conventions established by the Rust
conversions of the SwiftBar plugins in `~/Development/rust/SwiftbarPlugins/`
(`lamp_control_swiftbar_plugin`, `ssh_tunnels_swiftbar_plugin`).

## Source analysis (current Zig state)

- Version 0.1.1, Zig 0.15.1, single binary printing the server's IPv6 address
  to stdout.
- `src/main.zig`: allocates, gets the local IPv6 via `network/local_ethernet.zig`,
  prints it. All DNS/gist code is currently commented out.
- `src/network/local_ethernet.zig`: C FFI (`getifaddrs`, `inet_ntop`,
  `arpa/inet.h`) enumerating interfaces; picks the best global IPv6 address:
  - macOS: interface `en0`; Linux: first existing of a preferred list
    (`eth0`, `end0`, `enp0s0`, `enp1s0`, `eno1`, `ens33`, `en0`, `wlan0`),
    falling back to the first non-loopback interface with an IPv6 address.
  - Scopes: link-local < unique-local < global; skips unspecified, loopback,
    multicast and Linux temporary addresses; prefers non-deprecated, then
    longer prefix, then lexicographic order (see `shouldPrefer`).
  - Linux extras: reads `/proc/net/if_inet6` for dynamic/deprecated/temporary
    flags (via `addressStatusFromFlags`).
- `src/utilities/`: `logger.zig` (ANSI colored INFO/DEBUG/WARNING/ERROR/FATAL
  to stderr, DEBUG only in Debug builds), `print.zig` (flushed stdout/stderr).
- `config.json` read from a `.get-server-ip/` subdirectory next to the
  executable: gist URI + DDNS domain list (not yet consumed by the current
  code; part of the planned DNS feature).
- `build.zig`: cross-compilation targets `linux-x86_64`, `linux-aarch64`
  (rpi5), `rpi0` (arm1176jzf_s, ReleaseSmall), plus native macOS; plus
  `run` and `test` steps.
- TODO.md items: CLI help/usage/options, verbosity-gated logging, concurrent
  time-bounded retrieval from gist + DDNS sources, remove trailing NUL in
  output.
- Disabled modules (`*.zig_`): `config/config.zig`, `cli_interface.zig`,
  `dns/gist.zig`, `dns/dns.zig`, `dns/get_ip.zig` — the planned gist/DDNS
  checking feature.

## Conventions inherited from the SwiftBar plugin ports

- Rust edition 2024, `clap` (derive) for CLI, `serde`/`serde_json` for config,
  `libc` for POSIX FFI (precedent: `ssh_tunnels_swiftbar_plugin`).
- Module layout mirroring the Zig sources: `src/network/`, `src/dns/`,
  `src/utilities/{logger,print}.rs` with `mod.rs` re-exports.
- Size-optimized release profile (`strip`, `opt-level = "z"`, `lto`,
  `codegen-units = 1`, `panic = "abort"`).
- Unit tests inline in `#[cfg(test)] mod tests` at the bottom of each module.
- README badge style `![Rust version](https://img.shields.io/badge/Rust_version-edition_2024-green)`
  and a note "Initially developed in Zig, now implemented in Rust."

## Git strategy

The Rust code lives in this new folder (`~/Development/rust/get-server-ip`)
but on the existing GitHub repository `pvico/get-server-ip`, on a new branch
`rust`. The Zig implementation is preserved on branch `zig-old`, and `rust`
eventually becomes the default branch.

1. In this folder, connect the freshly `cargo init`-ed repo to the existing
   remote and create the `rust` branch from `origin/master` (shared history,
   clean later swap):

   ```bash
   git remote add origin git@github.com:pvico/get-server-ip.git
   git fetch origin
   git switch -c rust origin/master
   ```

2. On `rust`, remove the Zig sources (`build.zig`, `build.zig.zon`, `src/*.zig`,
   `.vscode/`, `zig-out/`, `.zig-cache/`) and add the Rust project files;
   commit.
3. During development, `master` keeps the Zig code untouched; all work is
   committed to `rust` and pushed as `origin/rust`.
4. When the port is finalized (feature-complete, tested, README updated):

   ```bash
   git switch master && git branch -m master zig-old
   git push origin zig-old
   git switch rust && git branch -m rust master
   git push origin master
   ```

5. On GitHub: set the default branch to `master` (Settings → Branches),
   delete the stale `origin/HEAD` pointer, optionally protect `master`.

## Implementation phases

### Phase 0 — Scaffold (this step, done)

- `cargo init --name get-server-ip` in `~/Development/rust/get-server-ip`.
- `Cargo.toml` per conventions: edition 2024, version 0.1.1, clap/serde/
  serde_json/libc, size-optimized release profile.
- This plan file (`PLAN.md`).

### Phase 1 — Port the current working functionality

- `src/utilities/print.rs`, `src/utilities/logger.rs`: direct ports of
  `print.zig` and `logger.zig` (same ANSI colors, stderr output, DEBUG gated
  by `#[cfg(debug_assertions)]`).
- `src/network/local_ethernet.rs`: port of `local_ethernet.zig` using
  `libc::getifaddrs`/`freeifaddrs`/`inet_ntop`; keep the interface preference
  lists, scope classification (`AddressScope`), status flags
  (`AddressStatus`), `/proc/net/if_inet6` parsing on Linux and the
  `shouldPrefer` ranking. Use `std::net::Ipv6Addr` and owned `String`/`Vec`
  instead of manual allocator juggling.
- `src/main.rs`: minimal flow — get the best local IPv6, print it to stdout
  (no trailing NUL; TODO item 1 of the Zig version), exit non-zero on failure.
- Unit tests for pure logic: `classifyIPv6`, `ipv6PrefixLength`,
  `shouldPrefer`, `AddressStatus::rank/format`.

### Phase 2 — CLI and configuration

- `src/cli_interface.rs`: clap-derive CLI (`-h/--help`, `-V/--version`,
  `-c/--config FILE_PATH`, verbosity flags), replacing `clap` (zig) — covers
  TODO items 2 and 3.
- `src/config/config.rs`: serde port of the disabled `config.zig`, reading
  `config.json` from `.<exe_name>/config.json` next to the executable or the
  path given via `--config`; schema stays `{ "gist-uri", "ddns-domains" }`.
- Include a checked-in `config.json` template as in the Zig project.

### Phase 3 — DNS and gist retrieval (the planned feature)

- `src/dns/gist.rs`: fetch the IP from the configured GitHub gist URI over
  HTTP(S) — `ureq` (default-features = false, features = ["gzip"]), same
  dependency style as `lamp_control_swiftbar_plugin`.
- `src/dns/dns.rs`: resolve each DDNS domain with `std::net::ToSocketAddrs`.
- `src/dns/get_ip.rs`: orchestration per README — run all sources concurrently
  and time-bounded (std threads + channels and receive timeouts, no async
  runtime, keeping the binary small); accept an IP when gist and at least one
  DDNS agree, otherwise when at least two DDNS agree; fall back to the local
  interface IP when no source is available.
- Port `main.zig`'s commented flow into `main.rs` behind the CLI.

### Phase 4 — Cross-compilation targets

Replace the Zig `build.zig` target matrix with cargo tooling (evaluate and
pick one):

- `cross` (Docker-based) or `cargo-zigbuild` (reuses the local zig for
  linking) for: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`
  (rpi5), `arm-unknown-linux-gnueabihf` with `arm1176jzf-s` CPU (rpi0,
  built with `--release` plus the size-optimized profile).
- Wrap the targets in a small build script or `.cargo/config.toml` aliases so
  the UX matches the old `zig build linux-x86_64`, `linux-aarch64`, `rpi0`.

### Phase 5 — Documentation and cleanup

- Rewrite `README.md` on the Rust conventions (edition 2024 badge, build
  instructions, configuration, usage, cross-compilation), keeping the
  "Initially developed in Zig, now implemented in Rust." note.
- `.gitignore`: `/target` plus `.DS_Store` and `.get-server-ip/`.
- `TODO.md`: refreshed for the Rust state (strike ported items, keep the
  remaining ones).
- Full `cargo test`, `cargo clippy`, `cargo fmt --check` green.

## Definition of done

- `cargo build --release` produces a `get-server-ip` binary printing the
  server IPv6 on macOS and Linux, byte-compatible with the Zig output.
- All phases merged on branch `rust`, pushed to `origin/rust`.
- Branch swap executed (Zig → `zig-old`, Rust → default `master`).
