# TODO

All items of the original Zig TODO list are covered by the Rust port:

1. Remove trailing '\00' character from output — done: the Rust port never
   emits a trailing NUL
2. CLI help, usage, options, etc. — done: clap (`-h/--help`, `-V/--version`,
   `-c/--config FILE_PATH`)
3. Print logger messages only when verbosity level is increased by CLI
   option — done: `-v` shows info, `-vv` shows debug
4. Make concurrent retrievals of the IP on the different sources and resolve
   as soon as sufficient sources have been retrieved — done: one thread per
   source, consensus check after every result
5. Time bound those retrievals — done: 5 s overall deadline, then fallback
   to the local interface IP

Remaining ideas:

1. Deploy step: install the built binary (and a config template) onto the
   target devices from `scripts/build-remote.sh`
2. Optional IPv4 DDNS sources for dual-stack setups
3. CI workflow running `cargo test`, `cargo clippy` and `cargo fmt --check`
