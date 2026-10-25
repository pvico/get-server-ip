//! Concurrent, time-bounded IP retrieval from the configured sources.
//! Rust port of the Zig `src/dns/get_ip.zig_` orchestration, implementing
//! TODO items 4 and 5 of the Zig version (concurrency + time bound): every
//! source runs on its own thread, results arrive over a channel, and the
//! first consensus wins:
//!
//! - the gist IP matching at least one DDNS IP, or
//! - at least two DDNS IPs matching each other.
//!
//! When no consensus is reached within the time bound, `None` is returned
//! and the caller falls back to the local interface IP. Unlike the Zig
//! version, the `0.0.0.0` guard is unnecessary: every source result is
//! validated as an IPv6 address before it participates in the comparison.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::utilities::logger;

use super::dns;
use super::gist;

/// Overall time bound for all sources together (Zig TODO item 5).
pub const GET_IP_TIMEOUT: Duration = Duration::from_secs(5);

/// One IP source: the gist or a single DDNS domain.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Source {
    Gist { uri: String },
    Dns { domain: String },
}

impl Source {
    /// Human readable name for logging and thread naming.
    fn describe(&self) -> String {
        match self {
            Source::Gist { .. } => "gist".to_string(),
            Source::Dns { domain } => domain.clone(),
        }
    }
}

/// What a source fetch produced; errors are carried as plain messages.
pub type SourceResult = Result<String, String>;

/// Fetch a source with the real implementations (gist over HTTP(S),
/// DDNS via the system resolver).
fn fetch_source(source: &Source) -> SourceResult {
    match source {
        Source::Gist { uri } => gist::get_ip(uri).map_err(|err| err.to_string()),
        Source::Dns { domain } => dns::resolve_ipv6(domain).map_err(|err| err.to_string()),
    }
}

/// The sources derived from the configuration, skipping unset values.
fn sources_from_config(config: &Config) -> Vec<Source> {
    let mut sources = Vec::new();
    if !config.gist_uri.is_empty() {
        sources.push(Source::Gist {
            uri: config.gist_uri.clone(),
        });
    }
    for domain in &config.ddns_domains {
        if !domain.is_empty() {
            sources.push(Source::Dns {
                domain: domain.clone(),
            });
        }
    }
    sources
}

/// Try to retrieve the server IP from the gist and DDNS sources.
/// Returns `None` when nothing was configured or no consensus was reached
/// within `timeout`, letting the caller fall back to the local interface.
pub fn get_ip(config: &Config, timeout: Duration) -> Option<String> {
    let sources = sources_from_config(config);
    if sources.is_empty() {
        logger::debug("No IP sources configured; falling back to the local interface");
        return None;
    }
    run(&sources, timeout, fetch_source)
}

/// Spawn one thread per source, collect results with a receive deadline and
/// return the first consensus IP. `fetch` is injectable so tests can drive
/// the consensus logic without network. Threads are detached on purpose: a
/// hung source (e.g. stuck resolver) must not extend the time bound of this
/// short-lived process.
fn run<F>(sources: &[Source], timeout: Duration, fetch: F) -> Option<String>
where
    F: Fn(&Source) -> SourceResult + Send + Sync + 'static,
{
    let (sender, receiver) = mpsc::channel::<(Source, SourceResult)>();
    let fetch = Arc::new(fetch);
    for source in sources {
        let sender = Sender::clone(&sender);
        let source = source.clone();
        let fetch = Arc::clone(&fetch);
        let _ = thread::Builder::new()
            .name(format!("get-ip-{}", source.describe()))
            .spawn(move || {
                let result = fetch(&source);
                let _ = sender.send((source, result));
            });
    }
    drop(sender); // so the channel disconnects once all sources have replied

    let deadline = Instant::now() + timeout;
    let mut gist_ip: Option<String> = None;
    let mut domain_ips: HashMap<String, String> = HashMap::new();

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match receiver.recv_timeout(remaining) {
            Ok((Source::Gist { .. }, Ok(ip))) => {
                logger::debug(&format!("Gist IP: {ip}"));
                // Gist and at least one DDNS agree.
                if domain_ips.values().any(|domain_ip| *domain_ip == ip) {
                    return Some(ip);
                }
                gist_ip = Some(ip);
            }
            Ok((Source::Dns { domain }, Ok(ip))) => {
                logger::debug(&format!("DNS IP for {domain}: {ip}"));
                // The DNS IP matches the gist IP...
                if gist_ip.as_deref() == Some(ip.as_str()) {
                    return Some(ip);
                }
                // ...or a second DDNS IP agrees with an earlier one.
                if domain_ips.values().any(|domain_ip| *domain_ip == ip) {
                    return Some(ip);
                }
                domain_ips.insert(domain, ip);
            }
            Ok((Source::Gist { .. }, Err(err))) => {
                logger::warn(&format!("Failed to get Github gist IP: {err}"));
            }
            Ok((Source::Dns { domain }, Err(err))) => {
                logger::warn(&format!("Failed to get DNS IP for domain {domain}: {err}"));
            }
            Err(mpsc::RecvTimeoutError::Timeout) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sources(gist: bool, domains: &[&str]) -> Vec<Source> {
        let mut sources = Vec::new();
        if gist {
            sources.push(Source::Gist {
                uri: "https://gist.example/x".to_string(),
            });
        }
        for domain in domains {
            sources.push(Source::Dns {
                domain: (*domain).to_string(),
            });
        }
        sources
    }

    fn fixed_fetch(mapping: HashMap<Source, String>) -> impl Fn(&Source) -> SourceResult {
        move |source| {
            // A little delay so results arrive in separate channel events.
            thread::sleep(Duration::from_millis(5));
            mapping
                .get(source)
                .cloned()
                .ok_or_else(|| "unmapped source".to_string())
        }
    }

    const TIMEOUT: Duration = Duration::from_secs(2);

    #[test]
    fn gist_and_dns_agree() {
        let sources = sources(true, &["a.example", "b.example"]);
        let mut mapping = HashMap::new();
        mapping.insert(sources[0].clone(), "2001:db8::1".to_string());
        mapping.insert(sources[1].clone(), "2001:db8::1".to_string());
        mapping.insert(sources[2].clone(), "2001:db8::2".to_string());

        let ip = run(&sources, TIMEOUT, fixed_fetch(mapping)).unwrap();
        assert_eq!(ip, "2001:db8::1");
    }

    #[test]
    fn gist_agreement_counts_whichever_arrives_first() {
        // DNS results arrive before the gist result: the gist check must
        // still fire when the gist response lands.
        let sources = sources(true, &["a.example"]);
        let mut mapping = HashMap::new();
        mapping.insert(sources[0].clone(), "2001:db8::7".to_string());
        mapping.insert(sources[1].clone(), "2001:db8::7".to_string());

        assert_eq!(
            run(&sources, TIMEOUT, fixed_fetch(mapping)).as_deref(),
            Some("2001:db8::7")
        );
    }

    #[test]
    fn two_dns_agree_without_gist() {
        let sources = sources(false, &["a.example", "b.example", "c.example"]);
        let mut mapping = HashMap::new();
        mapping.insert(sources[0].clone(), "2001:db8::1".to_string());
        mapping.insert(sources[1].clone(), "2001:db8::2".to_string());
        mapping.insert(sources[2].clone(), "2001:db8::2".to_string());

        assert_eq!(
            run(&sources, TIMEOUT, fixed_fetch(mapping)).as_deref(),
            Some("2001:db8::2")
        );
    }

    #[test]
    fn no_consensus_returns_none() {
        let sources = sources(true, &["a.example", "b.example"]);
        let mut mapping = HashMap::new();
        mapping.insert(sources[0].clone(), "::1".to_string());
        mapping.insert(sources[1].clone(), "2001:db8::1".to_string());
        mapping.insert(sources[2].clone(), "2001:db8::2".to_string());

        assert_eq!(run(&sources, TIMEOUT, fixed_fetch(mapping)), None);
    }

    #[test]
    fn timeout_cuts_off_slow_sources() {
        let sources = sources(true, &["slow.example"]);
        let started = Instant::now();
        let result = run(&sources, Duration::from_millis(100), |source| {
            thread::sleep(Duration::from_millis(600));
            Err(format!("slow source {source:?}"))
        });
        assert_eq!(result, None);
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "run() must not wait for slow sources"
        );
    }

    #[test]
    fn single_source_can_neither_confirm_nor_fail() {
        // One DDNS domain alone can never reach consensus.
        let sources = sources(false, &["a.example"]);
        let mut mapping = HashMap::new();
        mapping.insert(sources[0].clone(), "2001:db8::1".to_string());

        assert_eq!(run(&sources, TIMEOUT, fixed_fetch(mapping)), None);
    }
}
