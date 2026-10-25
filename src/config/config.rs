//! serde port of the disabled Zig `src/config/config.zig_` module.
//!
//! JSON shape (kebab-case keys, matching the Zig project's `config.json`
//! template — the Zig parser looked for snake_case keys and would never have
//! matched its own template):
//!
//! ```json
//! {
//!   "gist-uri": "https://gist.github.com/<user>/<id>",
//!   "ddns-domains": ["mysubdomain.example.com", "mysubdomain.example.org"]
//! }
//! ```
//!
//! By default the file is read from `.<exe_name>/config.json` next to the
//! executable (Zig `selfExeDirPath` + `.get-server-ip` + `config.json`); the
//! `-c/--config FILE_PATH` option overrides the location. Unknown fields are
//! rejected, mirroring the strict Zig `jsonParseFromValue`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::utilities::logger;

/// The parsed configuration. Missing fields fall back to the empty values of
/// the Zig `Config.init`. The canonical key spelling is kebab-case (matching
/// the Zig project's `config.json` template); the snake_case spellings of
/// the disabled Zig parser (`gist_uri`, `ddns_domains`) are accepted as
/// aliases so configs written for it keep working.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Config {
    /// GitHub gist URI holding the published IP address.
    #[serde(default, alias = "gist_uri")]
    pub gist_uri: String,

    /// Dynamic-DNS domains to resolve and compare against.
    #[serde(default, alias = "ddns_domains")]
    pub ddns_domains: Vec<String>,
}

/// Parses a configuration from a JSON string (Zig `fromJson`).
pub fn from_json(json: &str) -> io::Result<Config> {
    serde_json::from_str(json).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Loads and parses the configuration file (Zig `fromJson` + file reading).
pub fn load(path: &Path) -> io::Result<Config> {
    let contents = fs::read_to_string(path)
        .map_err(|e| annotate(path, "failed to read configuration file", e))?;

    from_json(&contents).map_err(|e| annotate(path, "failed to parse configuration file", e))
}

/// Default configuration location: `.<exe_name>/config.json` next to the
/// executable.
pub fn default_path() -> io::Result<PathBuf> {
    let exe_path = std::env::current_exe()?;
    Ok(exe_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".get-server-ip")
        .join("config.json"))
}

/// Loads the configuration: from `explicit` if given, otherwise from the
/// default location. Returns `None` when no explicit path was given and the
/// default file does not exist (the DNS feature is simply not configured).
/// Any other failure is fatal, matching the Zig `initFromJsonFile`.
pub fn init_from_json_file(explicit: Option<&Path>) -> Option<Config> {
    let path = match explicit {
        Some(path) => path.to_path_buf(),
        None => match default_path() {
            Ok(path) if path.exists() => path,
            Ok(_) => return None,
            Err(err) => {
                logger::fatal(&format!("Failed to get executable path: {err}"));
                std::process::exit(1);
            }
        },
    };

    match load(&path) {
        Ok(config) => {
            logger::debug(&format!("Gist URI: {}", config.gist_uri));
            for domain in &config.ddns_domains {
                logger::debug(&format!("DDNS Domain: {domain}"));
            }
            Some(config)
        }
        Err(err) => {
            logger::fatal(&format!(
                "Failed to load config file {}: {err}",
                path.display()
            ));
            std::process::exit(1);
        }
    }
}

/// Adds file context to an I/O error message.
fn annotate(path: &Path, context: &str, err: io::Error) -> io::Error {
    io::Error::new(
        err.kind(),
        ContextError {
            context: format!("{context} {}: {err}", path.display()),
        },
    )
}

#[derive(Debug)]
struct ContextError {
    context: String,
}

impl std::fmt::Display for ContextError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.context)
    }
}

impl std::error::Error for ContextError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_kebab_case_schema() {
        let config = from_json(
            r#"{
                "gist-uri": "https://gist.github.com/me/abc123",
                "ddns-domains": ["a.example.com", "b.example.org"]
            }"#,
        )
        .unwrap();
        assert_eq!(config.gist_uri, "https://gist.github.com/me/abc123");
        assert_eq!(config.ddns_domains, vec!["a.example.com", "b.example.org"]);
    }

    #[test]
    fn missing_fields_default_to_empty() {
        let config = from_json("{}").unwrap();
        assert_eq!(config.gist_uri, "");
        assert!(config.ddns_domains.is_empty());
    }

    #[test]
    fn accepts_snake_case_keys_from_zig_era_configs() {
        let config = from_json(
            r#"{
                "gist_uri": "https://gist.github.com/me/abc123",
                "ddns_domains": ["a.example.com"]
            }"#,
        )
        .unwrap();
        assert_eq!(config.gist_uri, "https://gist.github.com/me/abc123");
        assert_eq!(config.ddns_domains, vec!["a.example.com"]);
    }

    #[test]
    fn spellings_must_not_be_mixed() {
        // Both spellings of the same field in one file is a mistake.
        assert!(from_json(r#"{ "gist_uri": "a", "gist-uri": "b" }"#).is_err());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        assert!(from_json(r#"{ "nope": true }"#).is_err());
    }

    #[test]
    fn invalid_json_is_rejected() {
        assert!(from_json("not json").is_err());
    }

    #[test]
    fn default_path_ends_with_get_server_ip_config() {
        let path = default_path().unwrap();
        assert_eq!(path.file_name().unwrap(), "config.json");
        assert_eq!(
            path.parent().unwrap().file_name().unwrap(),
            ".get-server-ip"
        );
    }

    #[test]
    fn loads_checked_in_template() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config.json");
        let config = load(&path).expect("config.json template must load");
        assert!(config.gist_uri.contains("gist.github.com"));
        assert_eq!(config.ddns_domains.len(), 3);
        assert!(
            config
                .ddns_domains
                .iter()
                .all(|d| d.starts_with("mysubdomain."))
        );
    }

    #[test]
    fn load_annotates_missing_file() {
        let err = load(Path::new("/nonexistent/get-server-ip/config.json")).unwrap_err();
        assert!(
            err.to_string()
                .contains("failed to read configuration file")
        );
    }
}
