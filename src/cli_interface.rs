//! Command line interface, replacing the Zig `clap` dependency and the
//! disabled `src/cli_interface.zig_` sketch (TODO items 2 and 3 of the Zig
//! version): `-h/--help`, `-V/--version`, `-c/--config FILE_PATH` and
//! `-v/--verbose` counters.

use std::path::PathBuf;

use clap::Parser;

/// A utility to find the (dynamic) IPv6 address of a server.
#[derive(Debug, Parser)]
#[command(name = env!("CARGO_PKG_NAME"), version)]
pub struct Cli {
    /// Path to the configuration file (default: .get-server-ip/config.json next to the executable).
    #[arg(short, long, value_name = "FILE_PATH")]
    pub config: Option<PathBuf>,

    /// Increase the verbosity of the log output (-v: info, -vv: debug).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Parses the process arguments, printing help/version or a usage error and
/// exiting, like any conventional CLI.
pub fn parse() -> Cli {
    Cli::parse()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    #[test]
    fn defaults() {
        let cli = Cli::try_parse_from(["get-server-ip"]).unwrap();
        assert!(cli.config.is_none());
        assert_eq!(cli.verbose, 0);
    }

    #[test]
    fn parses_config_and_verbosity() {
        let cli = Cli::try_parse_from(["get-server-ip", "-c", "/tmp/x.json", "-vv"]).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("/tmp/x.json")));
        assert_eq!(cli.verbose, 2);

        let cli =
            Cli::try_parse_from(["get-server-ip", "--config", "y.json", "--verbose"]).unwrap();
        assert_eq!(cli.config, Some(PathBuf::from("y.json")));
        assert_eq!(cli.verbose, 1);
    }

    #[test]
    fn help_and_version_exit_without_running() {
        let err = Cli::try_parse_from(["get-server-ip", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        let err = Cli::try_parse_from(["get-server-ip", "--version"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayVersion);
    }

    #[test]
    fn unknown_options_are_rejected() {
        let err = Cli::try_parse_from(["get-server-ip", "--nope"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }
}
