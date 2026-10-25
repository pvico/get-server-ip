//! Fetch the published IP address from the configured GitHub gist.
//! Rust port of the Zig `src/dns/gist.zig_` module: GET the gist URI over
//! HTTP(S) and return its raw body. The Zig version wrote the response into
//! a fixed 16-byte buffer, so a trailing newline would already have broken
//! it; here the body is trimmed and validated as an IPv6 address, returning
//! its canonical RFC 5952 form so it compares cleanly with the other IP
//! sources.

use std::fmt;
use std::net::Ipv6Addr;
use std::time::Duration;

/// Per-request timeout for the gist fetch.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Errors returned by the gist fetch (Zig had a single `HttpError`).
#[derive(Debug)]
pub enum GistError {
    /// Transport failure or an unexpected HTTP status.
    Http(String),
    /// The gist content is not a valid IPv6 address.
    InvalidContent(String),
}

impl fmt::Display for GistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GistError::Http(message) => write!(f, "HTTP error: {message}"),
            GistError::InvalidContent(content) => {
                write!(f, "invalid gist content: {content:?}")
            }
        }
    }
}

impl std::error::Error for GistError {}

/// Fetch the gist at `uri` and return its content, trimmed, validated as an
/// IPv6 address and canonicalized.
pub fn get_ip(uri: &str) -> Result<String, GistError> {
    let response = agent()
        .get(uri)
        .header(
            "User-Agent",
            concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|err| GistError::Http(err.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(GistError::Http(format!("unexpected HTTP status {status}")));
    }

    let body = response
        .into_body()
        .read_to_string()
        .map_err(|err| GistError::Http(err.to_string()))?;

    parse_ip(&body).ok_or_else(|| GistError::InvalidContent(body.trim().to_string()))
}

/// Trim gist text and validate it as an IPv6 address, returning its
/// canonical RFC 5952 form.
fn parse_ip(text: &str) -> Option<String> {
    text.trim()
        .parse::<Ipv6Addr>()
        .ok()
        .map(|ip| ip.to_string())
}

/// Build a fresh HTTP agent with the request timeout applied.
/// ureq transparently decompresses gzip responses.
fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(REQUEST_TIMEOUT))
        .build()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_trimmed_canonical_ipv6() {
        assert_eq!(parse_ip("2001:db8::1").as_deref(), Some("2001:db8::1"));
        assert_eq!(parse_ip("  2001:DB8::1 \n").as_deref(), Some("2001:db8::1"));
        assert_eq!(parse_ip("fe80:0:0:0:0:0:0:1\n").as_deref(), Some("fe80::1"));
        assert_eq!(parse_ip("::").as_deref(), Some("::"));
    }

    #[test]
    fn rejects_non_ipv6_content() {
        assert_eq!(parse_ip("1.2.3.4"), None);
        assert_eq!(parse_ip("hello world"), None);
        assert_eq!(parse_ip(""), None);
    }
}
