/** The person's browsing history: committed top-level pages, bounded, searchable and private. */
use super::bookmarks::Bookmarks;
use crate::profile::{ProfileError, session_url_valid};
use serde::{Deserialize, Serialize};

/// Version rejects saved history whose meaning this build does not know.
pub const HISTORY_VERSION: u32 = 1;
/// The newest pages kept, as Oya's HISTORY_LIMIT bounds its library; older ones fall off.
pub const HISTORY_LIMIT: usize = 1000;
/// One Keychain record holds the whole history, so its encoded size is bounded too.
pub const HISTORY_BYTES: usize = 512 * 1024;
/// Titles are page-provided text; a long one cannot grow the record or a menu row.
pub const TITLE_CHARS: usize = 200;
/// The address field's suggestion list stays small enough for short windows, as Oya's.
pub const SUGGESTION_LIMIT: usize = 8;
/// Rows one site may take among the suggestions, so other sites still show.
const PER_SITE: usize = 3;
/// Typed queries longer than this are not addresses anyone is completing.
const QUERY_BYTES: usize = 500;
/// Match weights, Oya's order: site name, then address, then a title word; a bookmark edges history.
const SITE: u32 = 1000;
const HOME: u32 = 500;
const ADDRESS: u32 = 800;
const WORD: u32 = 300;
const BOOKMARK: u32 = 50;

/** One page the person visited: its address, its title then, and when (Unix seconds). */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Visit {
    /// A credential-free HTTP(S) address, already stripped of a protected query.
    pub url: String,
    /// The document title when it committed, capped.
    pub title: String,
    /// Seconds since the Unix epoch of the latest visit.
    pub time: u64,
}

/** History newest first, one row per address so a revisit moves it to the top. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct History {
    /// Format version, checked on every read.
    pub version: u32,
    /// Newest first.
    pub visits: Vec<Visit>,
}

impl Default for History {
    /** A fresh profile has no history at the current version. */
    fn default() -> Self {
        Self {
            version: HISTORY_VERSION,
            visits: Vec::new(),
        }
    }
}

/** One address-field completion, from history or a bookmark. */
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suggestion {
    /// The address that opens when the person picks it.
    pub url: String,
    /// The title shown beside it.
    pub title: String,
    /// Bookmarks are marked with a star.
    pub bookmark: bool,
}

impl History {
    /** Record a committed page; addresses Mars may not keep (not HTTP(S), credentials) are skipped. */
    pub fn record(&mut self, url: &str, title: &str, time: u64) -> bool {
        if !session_url_valid(url) {
            return false;
        }
        self.visits.retain(|visit| visit.url != url);
        self.visits.insert(
            0,
            Visit {
                url: url.into(),
                title: cap(title),
                time,
            },
        );
        self.visits.truncate(HISTORY_LIMIT);
        while self.visits.len() > 1 && encoded_len(self) > HISTORY_BYTES {
            self.visits.pop();
        }
        true
    }

    /** Positions of pages whose title or address contains every word of `query`, newest first; empty matches all. */
    pub fn search(&self, query: &str) -> Vec<usize> {
        let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        self.visits
            .iter()
            .enumerate()
            .filter(|(_, visit)| {
                let text = format!("{} {}", visit.title, visit.url).to_lowercase();
                words.iter().all(|word| text.contains(word))
            })
            .map(|(index, _)| index)
            .collect()
    }

    /** Forget every page; the caller persists the empty record. */
    pub fn clear(&mut self) {
        self.visits.clear();
    }

    /** Serialize after checking every bound a read would check. */
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        let valid = self.version == HISTORY_VERSION
            && self.visits.len() <= HISTORY_LIMIT
            && self.visits.iter().all(|visit| {
                session_url_valid(&visit.url) && visit.title.chars().count() <= TITLE_CHARS
            });
        if !valid {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > HISTORY_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /** Read a saved record as untrusted input: strict fields, version, sizes and addresses. */
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > HISTORY_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let history: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        history.encode()?;
        Ok(history)
    }
}

/** Address-field completions, Oya's way: a site whose name starts with the text first, its home page before deep links; then addresses starting with it; then a title word. Bookmarks edge out history, no site takes more than three rows, at most eight. */
pub fn suggestions(history: &History, bookmarks: &Bookmarks, typed: &str) -> Vec<Suggestion> {
    let query = bare(typed.trim());
    if query.is_empty() || typed.len() > QUERY_BYTES {
        return Vec::new();
    }
    let candidates = bookmarks
        .items
        .iter()
        .map(|item| (item.url.as_str(), item.title.as_str(), true))
        .chain(
            history
                .visits
                .iter()
                .map(|visit| (visit.url.as_str(), visit.title.as_str(), false)),
        );
    let mut scored: Vec<(u32, usize, Suggestion)> = Vec::new();
    for (order, (url, title, bookmark)) in candidates.enumerate() {
        let score = score(url, title, &query);
        if score == 0 || scored.iter().any(|(_, _, seen)| seen.url == url) {
            continue;
        }
        let score = score + if bookmark { BOOKMARK } else { 0 };
        let suggestion = Suggestion {
            url: url.into(),
            title: title.into(),
            bookmark,
        };
        scored.push((score, order, suggestion));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    spread(scored.into_iter().map(|(_, _, item)| item))
}

/** How well one address and title answer the typed text; zero when they do not. */
fn score(url: &str, title: &str, query: &str) -> u32 {
    let address = bare(url);
    let host = address.split(['/', '?', '#']).next().unwrap_or_default();
    if host.starts_with(query) {
        let home = address[host.len()..].trim_end_matches('/').is_empty();
        return SITE + if home { HOME } else { 0 };
    }
    if address.starts_with(query) {
        return ADDRESS;
    }
    let title = title.to_lowercase();
    if title
        .split(|c: char| !c.is_alphanumeric())
        .any(|word| word.starts_with(query))
    {
        return WORD;
    }
    0
}

/** At most PER_SITE rows from any one site, in rank order, and at most SUGGESTION_LIMIT rows. */
fn spread(items: impl Iterator<Item = Suggestion>) -> Vec<Suggestion> {
    let mut seen: Vec<(String, usize)> = Vec::new();
    let mut result = Vec::new();
    for item in items {
        let address = bare(&item.url);
        let host = address.split(['/', '?', '#']).next().unwrap_or_default();
        let count = match seen.iter_mut().find(|(site, _)| site == host) {
            Some((_, count)) => count,
            None => {
                seen.push((host.into(), 0));
                &mut seen.last_mut().expect("just pushed").1
            }
        };
        *count += 1;
        if *count <= PER_SITE {
            result.push(item);
        }
        if result.len() == SUGGESTION_LIMIT {
            break;
        }
    }
    result
}

/** An address as people type it: lowercase, without the scheme or a leading `www.`. */
fn bare(url: &str) -> String {
    let lower = url.to_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))
        .unwrap_or(&lower);
    rest.strip_prefix("www.").unwrap_or(rest).to_owned()
}

/** Cap page-provided titles by characters, never splitting one. */
pub(super) fn cap(title: &str) -> String {
    title.trim().chars().take(TITLE_CHARS).collect()
}

/** The size a record would encode to, for trimming the oldest pages until it fits. */
fn encoded_len(history: &History) -> usize {
    serde_json::to_vec(history).map_or(usize::MAX, |bytes| bytes.len())
}
