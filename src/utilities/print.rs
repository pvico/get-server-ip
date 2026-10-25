//! Utility functions for printing to standard output and error.
//! Mirrors the Zig `src/utilities/print.zig` module: every call writes and
//! flushes in one go so output is never interleaved or lost on exit.

use std::io::{self, Write};

/// Print text to standard output.
pub fn out(text: &str) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    lock.write_all(text.as_bytes())?;
    lock.flush()
}

/// Print text to standard error.
pub fn err(text: &str) -> io::Result<()> {
    let stderr = io::stderr();
    let mut lock = stderr.lock();
    lock.write_all(text.as_bytes())?;
    lock.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn out_and_err_do_not_panic() {
        out("").unwrap();
        err("").unwrap();
    }
}
