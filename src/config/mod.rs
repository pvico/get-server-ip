// The nested `config::config` mirrors the Zig `src/config/config.zig_` layout.
#[allow(clippy::module_inception)]
pub mod config;

pub use config::{Config, init_from_json_file};
