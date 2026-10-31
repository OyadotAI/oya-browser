/**
 * Cache Storage: named caches per storage key, each an ordered list of request/response pairs,
 * persisted whole in the profile's CacheStorage record. Matching follows the Service Workers
 * spec's "Request Matches Cached Item": the URL without its fragment (and without its query under
 * ignoreSearch), GET unless ignoreMethod, and every request header the cached response's Vary
 * names unless ignoreVary; `Vary: *` never matches.
 */
use super::{Ctx, Reply, fail, ok, ok_bytes};
use crate::profile::Record;
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod handlers;
pub(super) use handlers::HANDLERS;

/** The request half of an entry: what matching reads. */
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct CachedRequest {
    pub url: String,
    pub method: String,
    pub headers: Vec<(String, String)>,
}

/** The response half of an entry, its body base64 so the record stays compact JSON. */
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct CachedResponse {
    pub url: String,
    pub status: u16,
    #[serde(rename = "statusText")]
    pub status_text: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
}

/** One stored pair. */
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct Entry {
    pub request: CachedRequest,
    pub response: CachedResponse,
}

/** One named cache. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(super) struct NamedCache {
    pub id: u64,
    pub name: String,
    pub entries: Vec<Entry>,
}

/** CacheQueryOptions. */
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Query {
    pub ignore_search: bool,
    pub ignore_method: bool,
    pub ignore_vary: bool,
}

impl Query {
    /** The options a page sent. */
    pub fn from(args: &Value) -> Self {
        Self {
            ignore_search: args["ignoreSearch"].as_bool() == Some(true),
            ignore_method: args["ignoreMethod"].as_bool() == Some(true),
            ignore_vary: args["ignoreVary"].as_bool() == Some(true),
        }
    }
}

/** Every storage key's caches, in creation order, and the next cache id. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::runtime) struct CacheStorage {
    partitions: BTreeMap<String, Vec<NamedCache>>,
    next_id: u64,
}

/** A URL without its fragment, and without its query when `ignore_search`. */
fn comparable(url: &str, ignore_search: bool) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return url.to_string();
    };
    parsed.set_fragment(None);
    if ignore_search {
        parsed.set_query(None);
    }
    parsed.to_string()
}

/** One header's value (case-insensitive name), joined as Fetch combines repeats. */
fn header(headers: &[(String, String)], name: &str) -> Option<String> {
    let values: Vec<&str> = headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
        .collect();
    (!values.is_empty()).then(|| values.join(", "))
}

/** "Request Matches Cached Item". */
pub(super) fn matches(query: &CachedRequest, entry: &Entry, options: Query) -> bool {
    if !options.ignore_method && query.method != "GET" {
        return false;
    }
    if comparable(&query.url, options.ignore_search)
        != comparable(&entry.request.url, options.ignore_search)
    {
        return false;
    }
    if options.ignore_vary {
        return true;
    }
    let Some(vary) = header(&entry.response.headers, "vary") else {
        return true;
    };
    vary.split(',')
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .all(|field| {
            field != "*" && header(&query.headers, field) == header(&entry.request.headers, field)
        })
}

impl CacheStorage {
    /** The caches of one storage key. */
    pub(super) fn caches(&mut self, key: &str) -> &mut Vec<NamedCache> {
        self.partitions.entry(key.into()).or_default()
    }
    /** The cache with this id in this key, if it still exists. */
    pub(super) fn cache(&mut self, key: &str, id: u64) -> Option<&mut NamedCache> {
        self.caches(key).iter_mut().find(|cache| cache.id == id)
    }
    /** Open (or create) a cache by name, returning its id. */
    pub(super) fn open(&mut self, key: &str, name: &str) -> u64 {
        if let Some(cache) = self.caches(key).iter().find(|cache| cache.name == name) {
            return cache.id;
        }
        self.next_id += 1;
        let id = self.next_id;
        self.caches(key).push(NamedCache {
            id,
            name: name.into(),
            entries: Vec::new(),
        });
        id
    }
}

impl NamedCache {
    /** Entries matching `query` (all of them when None), in order. */
    pub(super) fn lookup(&self, query: Option<&CachedRequest>, options: Query) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|entry| query.is_none_or(|query| matches(query, entry, options)))
            .collect()
    }
    /** Put: replace entries the request matches, then append. */
    pub(super) fn put(&mut self, entry: Entry) {
        let request = entry.request.clone();
        self.entries
            .retain(|old| !matches(&request, old, Query::default()));
        self.entries.push(entry);
    }
    /** Delete entries matching; true when any went. */
    pub(super) fn delete(&mut self, query: &CachedRequest, options: Query) -> bool {
        let before = self.entries.len();
        self.entries.retain(|old| !matches(query, old, options));
        before != self.entries.len()
    }
}

/** Responses as the page rebuilds them: records with lengths, bodies concatenated in order. */
pub(super) fn responses(entries: &[&Entry]) -> (Value, Vec<u8>) {
    let mut bytes = Vec::new();
    let mut records = Vec::new();
    for entry in entries {
        let body = crate::runtime::images::decode_base64(&entry.response.body).unwrap_or_default();
        let response = &entry.response;
        records.push(
            json!({"url":response.url,"status":response.status,"statusText":response.status_text,
            "type":response.kind,"headers":response.headers,"length":body.len()}),
        );
        bytes.extend(body);
    }
    (Value::Array(records), bytes)
}

impl Runtime {
    /** Save a candidate store, adopting it only when the profile accepted it. */
    pub(super) fn commit_caches(&mut self, next: CacheStorage) -> EngineResult<Reply> {
        if !super::records::Records::fits(Record::CacheStorage, &next) {
            return fail(
                "QuotaExceededError",
                "The cache storage quota has been exceeded.",
            );
        }
        if !self.platform.records.save(Record::CacheStorage, &next) {
            return fail("UnknownError", "The cache could not be saved.");
        }
        self.platform.caches = next;
        ok(Value::Null)
    }
}

/** The storage key a context's caches live under. */
pub(super) fn key(ctx: &Ctx) -> String {
    ctx.storage_key()
}

/** `ok_bytes` for a list of matched entries. */
pub(super) fn answer(entries: &[&Entry]) -> EngineResult<Reply> {
    let (records, bytes) = responses(entries);
    ok_bytes(records, bytes)
}

#[cfg(test)]
mod tests;
