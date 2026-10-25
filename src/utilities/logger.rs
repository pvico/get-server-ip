//! Logger utility for standardized logging with color-coded messages.
//! Mirrors the Zig `src/utilities/logger.zig` module and its ANSI colors:
//! info green (32), debug magenta (35), warning yellow (33), error bright red
//! (91) and fatal white-on-red (97;41).

use super::print;

/// Log an informational message (stdout, green "INFO" prefix).
#[allow(dead_code)] // parity with the Zig logger API
pub fn info(message: &str) {
    let _ = print::out(&format!("\x1b[32mINFO {message}\x1b[0m\n"));
}

/// Log a debug message (stderr, magenta "DEBUG" prefix).
/// Only printed when the program is compiled in debug mode.
#[allow(dead_code)] // parity with the Zig logger API
pub fn debug(message: &str) {
    if !cfg!(debug_assertions) {
        return;
    }
    let _ = print::err(&format!("\x1b[35mDEBUG {message}\x1b[0m\n"));
}

/// Log a warning message (stderr, yellow "WARNING" prefix).
pub fn warn(message: &str) {
    let _ = print::err(&format!("\x1b[33mWARNING {message}\x1b[0m\n"));
}

/// Log an error message (stderr, bright red "ERROR" prefix).
#[allow(dead_code)] // parity with the Zig logger API
pub fn err(message: &str) {
    let _ = print::err(&format!("\x1b[91mERROR {message}\x1b[0m\n"));
}

/// Log a fatal error message (stderr, white on red "FATAL" prefix).
pub fn fatal(message: &str) {
    let _ = print::err(&format!("\x1b[97;41mFATAL {message}\x1b[0m\n"));
}
