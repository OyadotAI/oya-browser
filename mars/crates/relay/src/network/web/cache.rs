/** A private HTTP cache (RFC 9111 subset), partitioned by top-level site, in memory and on disk. */
mod disk;
mod freshness;
use crate::network::WebRequest;
use crate::network::constants::{CACHE_MEMORY_BYTES, CACHE_VARIANTS};
pub use disk::set_cache_directory;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::SystemTime;

/**
 * One cached resource. The partition is the top-level site's origin, so a page on one site
 * can never observe, by content or timing, what another site's pages fetched. The fixture
 * grant and image budget are part of the key, so neither a grant change nor a different body
 * limit can reach an entry stored under the other.
 */
#[derive(Clone, Debug, Deserialize, Hash, PartialEq, Serialize)]
pub(super) struct Key {
    /// Top-level site origin, already validated by policy.
    partition: String,
    /// Absolute URL without fragment.
    url: String,
    /// Whether the loopback fixture grant admitted the fetch.
    fixture: bool,
    /// Image requests have a smaller body budget than documents.
    image: bool,
    /// The Origin the request sent, if any. A server that adds CORS headers only when asked and
    /// sends no `Vary: Origin` (BBC, nytimes) must not have its no-Origin copy answer a CORS fetch.
    #[serde(default)]
    origin: Option<String>,
}

impl Key {
    /** Only GETs carrying a partition are cacheable; everything else, preflights included, bypasses
    the cache (preflights have their own cache in the coordinator). */
    pub(super) fn of(request: &WebRequest, url: &::url::Url, fixture: bool) -> Option<Self> {
        if request.preflight.is_some() {
            return None;
        }
        let partition = request.top_level.as_deref()?;
        let partition = crate::network::policy::web_url(partition).ok()?;
        let mut url = url.clone();
        url.set_fragment(None);
        Some(Self {
            partition: partition.origin().ascii_serialization(),
            url: url.into(),
            fixture,
            image: request.image,
            origin: request.origin.clone(),
        })
    }
}

/** A stored 200 response without cookies or framing, plus what its reuse depends on. */
#[derive(Clone, Deserialize, Serialize)]
pub(super) struct Stored {
    /// Which request this answers.
    key: Key,
    /// Response headers minus Set-Cookie, framing and hop-by-hop fields.
    pub headers: BTreeMap<String, String>,
    /// Decoded body; written to disk after the metadata rather than inside the JSON.
    #[serde(skip)]
    pub body: Vec<u8>,
    /// Request header values for each Vary name, as sent when stored.
    vary: Vec<(String, String)>,
    /// Address the response came from; rechecked against the destination policy on each use.
    pub address: IpAddr,
    /// Unix seconds at receipt.
    stored_at: u64,
    /// Seconds old at receipt.
    initial_age: u64,
    /// Seconds of freshness.
    lifetime: u64,
    /// Revalidate before every use.
    no_cache: bool,
}

impl Stored {
    /** RFC 9111 section 4.2: current age below the freshness lifetime, and not `no-cache`. */
    pub(super) fn fresh(&self, now: SystemTime) -> bool {
        let resident = freshness::seconds(now).saturating_sub(self.stored_at);
        !self.no_cache && self.initial_age.saturating_add(resident) < self.lifetime
    }

    /** If-None-Match and If-Modified-Since fields for a conditional request, possibly none. */
    pub(super) fn validators(&self) -> Vec<(&'static str, String)> {
        let mut fields = Vec::new();
        if let Some(tag) = self.headers.get("etag") {
            fields.push(("if-none-match", tag.clone()));
        }
        if let Some(modified) = self.headers.get("last-modified") {
            fields.push(("if-modified-since", modified.clone()));
        }
        fields
    }

    /** Memory charged against the cache budget. */
    fn bytes(&self) -> usize {
        self.body.len()
            + self
                .headers
                .iter()
                .map(|(name, value)| name.len() + value.len())
                .sum::<usize>()
    }
}

/** Fields never stored: cookies go through the jar each time, framing describes old bytes. */
const UNSTORED: [&str; 7] = [
    "set-cookie",
    "connection",
    "keep-alive",
    "transfer-encoding",
    "content-length",
    "content-encoding",
    "age",
];

/** Least recently used entries first; the byte total is kept alongside. */
struct Memory {
    entries: Vec<Stored>,
    bytes: usize,
}

/**
 * ponytail: lookups scan a Vec; the byte budget keeps it to thousands of entries at most, a few
 * microseconds. Index by key if profiles show it.
 */
static MEMORY: Mutex<Memory> = Mutex::new(Memory {
    entries: Vec::new(),
    bytes: 0,
});

/** What the cache can do for a request. */
pub(super) enum Lookup {
    /// Serve without the network.
    Fresh(Stored),
    /// Send a conditional request; a 304 reuses this body.
    Stale(Stored),
    /// Fetch normally.
    Miss,
}

/**
 * Finds the entry whose Vary values match this request, memory first, then disk. An entry
 * from an address the destination policy no longer admits is treated as absent.
 */
pub(super) fn lookup(key: &Key, request: &WebRequest, now: SystemTime) -> Lookup {
    let found = memory_find(key, request).or_else(|| {
        let stored = disk::load(key).filter(|stored| matches(stored, request))?;
        remember(stored.clone());
        Some(stored)
    });
    match found {
        Some(stored) if !super::transport::admitted(stored.address, key.fixture) => Lookup::Miss,
        Some(stored) if stored.fresh(now) => Lookup::Fresh(stored),
        Some(stored) if !stored.validators().is_empty() => Lookup::Stale(stored),
        _ => Lookup::Miss,
    }
}

/**
 * Stores a 200 GET response. Refused: `no-store`, `Vary: *`, and responses that could
 * never be reused (no freshness and no validator).
 */
pub(super) fn store(
    key: Key,
    request: &WebRequest,
    headers: &BTreeMap<String, String>,
    body: &[u8],
    address: IpAddr,
    now: SystemTime,
) {
    let policy = freshness::policy(headers, now);
    let names = vary_names(headers);
    if policy.no_store || names.iter().any(|name| name == "*") {
        return;
    }
    let stored = Stored {
        vary: names
            .into_iter()
            .map(|name| {
                let value = request_field(request, &name);
                (name, value)
            })
            .collect(),
        headers: headers
            .iter()
            .filter(|(name, _)| !UNSTORED.contains(&name.as_str()))
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect(),
        body: body.to_vec(),
        key,
        address,
        stored_at: freshness::seconds(now),
        initial_age: policy.initial_age,
        lifetime: policy.lifetime,
        no_cache: policy.no_cache,
    };
    if stored.fresh(now) || !stored.validators().is_empty() {
        disk::save(&stored);
        remember(stored);
    }
}

/**
 * A 304 confirms the stored body: its headers replace the stored ones (section 4.3.4) and
 * freshness restarts from now. Returns the updated entry to serve.
 */
pub(super) fn freshen(
    mut stored: Stored,
    request: &WebRequest,
    headers: &BTreeMap<String, String>,
    now: SystemTime,
) -> Stored {
    let mut merged = stored.headers.clone();
    for (name, value) in headers {
        if !UNSTORED.contains(&name.as_str()) {
            merged.insert(name.clone(), value.clone());
        }
    }
    let key = stored.key.clone();
    let body = std::mem::take(&mut stored.body);
    store(key, request, &merged, &body, stored.address, now);
    stored.headers = merged;
    stored.body = body;
    stored
}

/// Empties the HTTP cache, memory and disk alike, for "Clear browsing data". Later requests
/// fetch afresh; the disk directory, if one is set, stays configured.
pub fn clear_cache() {
    if let Ok(mut memory) = MEMORY.lock() {
        memory.entries.clear();
        memory.bytes = 0;
    }
    disk::clear();
}

/** RFC 9111 section 4.4: an unsafe request to a URL invalidates every copy stored for it in this
partition, whatever Origin, grant or body budget each copy was stored under: a same-origin PUT
sends an Origin header, and matching the whole key left the plain GET's copy served stale. */
pub(super) fn invalidate(key: &Key) {
    remove_where(|stored| stored.partition == key.partition && stored.url == key.url);
    disk::remove(key);
}

/** Drops every copy of these URLs in every partition. Each network process keeps its own memory
cache, so the coordinator relays one process's unsafe request to the others with this. */
pub fn forget(urls: &[String]) {
    remove_where(|stored| urls.contains(&stored.url));
}

/** Remove the memory entries whose key matches, keeping the byte count exact. */
fn remove_where(matches: impl Fn(&Key) -> bool) {
    if let Ok(mut memory) = MEMORY.lock() {
        let removed: usize = memory
            .entries
            .iter()
            .filter(|entry| matches(&entry.key))
            .map(Stored::bytes)
            .sum();
        memory.entries.retain(|entry| !matches(&entry.key));
        memory.bytes -= removed;
    }
}

/** Memory hit, moved to the most recently used end. */
fn memory_find(key: &Key, request: &WebRequest) -> Option<Stored> {
    let mut memory = MEMORY.lock().ok()?;
    let index = memory
        .entries
        .iter()
        .rposition(|entry| entry.key == *key && matches(entry, request))?;
    let entry = memory.entries.remove(index);
    memory.entries.push(entry.clone());
    Some(entry)
}

/** Inserts into memory, replacing the same variant and evicting until within budget. */
fn remember(stored: Stored) {
    let Ok(mut memory) = MEMORY.lock() else {
        return;
    };
    let variants = memory
        .entries
        .iter()
        .filter(|entry| entry.key == stored.key)
        .count();
    let same = memory
        .entries
        .iter()
        .position(|entry| entry.key == stored.key && entry.vary == stored.vary)
        .or_else(|| {
            (variants >= CACHE_VARIANTS)
                .then(|| {
                    memory
                        .entries
                        .iter()
                        .position(|entry| entry.key == stored.key)
                })
                .flatten()
        });
    if let Some(index) = same {
        let old = memory.entries.remove(index);
        memory.bytes -= old.bytes();
    }
    if stored.bytes() > CACHE_MEMORY_BYTES {
        return;
    }
    memory.bytes += stored.bytes();
    memory.entries.push(stored);
    while memory.bytes > CACHE_MEMORY_BYTES {
        let old = memory.entries.remove(0);
        memory.bytes -= old.bytes();
    }
}

/** Every Vary name recorded at store time has the same request value now. */
fn matches(stored: &Stored, request: &WebRequest) -> bool {
    stored
        .vary
        .iter()
        .all(|(name, value)| request_field(request, name) == *value)
}

/** Lowercase Vary field names; `*` is kept so the caller can refuse it. */
fn vary_names(headers: &BTreeMap<String, String>) -> Vec<String> {
    headers.get("vary").map_or_else(Vec::new, |value| {
        value
            .split(',')
            .map(|name| name.trim().to_ascii_lowercase())
            .filter(|name| !name.is_empty())
            .collect()
    })
}

/** The value this request sends for a header; headers Relay never sends count as empty. */
fn request_field(request: &WebRequest, name: &str) -> String {
    super::fields::sent(request, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    /** Vary comparisons see configured preferences and page overrides exactly as the wire does. */
    #[test]
    fn language_variants_use_the_emitted_header() {
        let mut request = WebRequest::get("https://languages.test/");
        request.languages = vec!["fr".into()];
        assert_eq!(request_field(&request, "accept-language"), "fr");
        request.languages = vec!["ja".into()];
        assert_ne!(request_field(&request, "accept-language"), "fr");
        request.destination = crate::network::Destination::Empty;
        request.headers = vec![("accept-language".into(), "de".into())];
        assert_eq!(request_field(&request, "accept-language"), "de");
    }

    /** Filling memory past its budget evicts least recently used entries first. */
    #[test]
    fn memory_stays_within_its_byte_budget() {
        const BODY: usize = 1024 * 1024;
        let entries = CACHE_MEMORY_BYTES / BODY + 4;
        let key = |index: usize| Key {
            partition: "https://memory.test".into(),
            url: format!("https://memory.test/{index}"),
            fixture: false,
            image: false,
            origin: None,
        };
        for index in 0..entries {
            remember(Stored {
                key: key(index),
                headers: BTreeMap::new(),
                body: vec![0; BODY],
                vary: Vec::new(),
                address: IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
                stored_at: 0,
                initial_age: 0,
                lifetime: 0,
                no_cache: false,
            });
        }
        let memory = MEMORY.lock().expect("memory");
        assert!(memory.bytes <= CACHE_MEMORY_BYTES);
        assert!(
            !memory.entries.iter().any(|entry| entry.key == key(0)),
            "oldest evicted"
        );
        assert!(
            memory
                .entries
                .iter()
                .any(|entry| entry.key == key(entries - 1))
        );
    }
}
