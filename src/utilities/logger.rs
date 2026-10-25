//! Logger utility for standardized logging with color-coded messages.
//! Mirrors the Zig `src/utilities/logger.zig` module and its ANSI colors:
//! info green (32), debug magenta (35), warning yellow (33), error bright red
//! (91) and fatal white-on-red (97;41).
//!
//! Message visibility is gated by the verbosity level raised on the CLI
//! (`-v`: info, `-vv`: debug) — TODO item 3 of the Zig version. Warnings,
//! errors and fatal messages always print. Debug builds start fully verbose,
//! preserving the Zig DEBUG_MODE behavior during development.

use std::sync::atomic::{AtomicU8, Ordering};

use super::print;

/// Current verbosity: 0 = warn/error/fatal, 1 = +info, 2 = +debug.
static VERBOSITY: AtomicU8 = AtomicU8::new(0);

/// Raise (or lower) the verbosity level, e.g. from the CLI `-v` counter.
pub fn set_verbosity(level: u8) {
    VERBOSITY.store(level, Ordering::Relaxed);
}

/// The current verbosity level.
pub fn verbosity() -> u8 {
    VERBOSITY.load(Ordering::Relaxed)
}

/// Log an informational message (stdout, green "INFO" prefix).
/// Printed when verbosity is at least 1.
#[allow(dead_code)] // parity with the Zig logger API
pub fn info(message: &str) {
    if verbosity() < 1 {
        return;
    }
    let _ = print::out(&format!("\x1b[32mINFO {message}\x1b[0m\n"));
}

/// Log a debug message (stderr, magenta "DEBUG" prefix).
/// Printed when verbosity is at least 2 (debug builds start at 2).
#[allow(dead_code)] // parity with the Zig logger API
pub fn debug(message: &str) {
    if verbosity() < 2 {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbosity_roundtrip() {
        set_verbosity(2);
        assert_eq!(verbosity(), 2);
        // All log levels can be called at any verbosity without panicking.
        info("test");
        debug("test");
        warn("test");
        err("test");
        fatal("test");
        set_verbosity(0);
        assert_eq!(verbosity(), 0);
        info("suppressed");
        debug("suppressed");
        warn("visible");
    }
}
