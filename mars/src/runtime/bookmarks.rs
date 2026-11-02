/** The person's bookmarks: saved addresses with editable titles, bounded and private. */
use super::history::cap;
use crate::profile::{ProfileError, session_url_valid};
use serde::{Deserialize, Serialize};

/// Version rejects saved bookmarks whose meaning this build does not know.
pub const BOOKMARKS_VERSION: u32 = 1;
/// The most bookmarks one profile keeps; Cmd+D on a full list says so instead of dropping one.
pub const BOOKMARKS_LIMIT: usize = 500;
/// One Keychain record holds them all, so its encoded size is bounded too.
pub const BOOKMARKS_BYTES: usize = 256 * 1024;

/** One saved page. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bookmark {
    /// A credential-free HTTP(S) address.
    pub url: String,
    /// The page title when saved, or the person's own name for it.
    pub title: String,
}

/** Bookmarks in the order the person added them, as a bookmarks bar shows them. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Bookmarks {
    /// Format version, checked on every read.
    pub version: u32,
    /// Oldest first.
    pub items: Vec<Bookmark>,
}

impl Default for Bookmarks {
    /** A fresh profile has no bookmarks at the current version. */
    fn default() -> Self {
        Self {
            version: BOOKMARKS_VERSION,
            items: Vec::new(),
        }
    }
}

impl Bookmarks {
    /** Whether this address is bookmarked; the star in the address field reads it. */
    pub fn contains(&self, url: &str) -> bool {
        self.items.iter().any(|item| item.url == url)
    }

    /** Cmd+D: bookmark the page, or remove it when it already is. Returns whether it is bookmarked now; addresses Mars may not keep are refused. */
    pub fn toggle(&mut self, url: &str, title: &str) -> Result<bool, ProfileError> {
        if let Some(index) = self.items.iter().position(|item| item.url == url) {
            self.items.remove(index);
            return Ok(false);
        }
        if !session_url_valid(url) || self.items.len() >= BOOKMARKS_LIMIT {
            return Err(ProfileError::Corrupt);
        }
        let title = if title.trim().is_empty() { url } else { title };
        self.items.push(Bookmark {
            url: url.into(),
            title: cap(title),
        });
        if self.encode().is_err() {
            self.items.pop();
            return Err(ProfileError::Corrupt);
        }
        Ok(true)
    }

    /** Give one bookmark the person's own name; an empty name keeps the old one. */
    pub fn rename(&mut self, index: usize, title: &str) -> bool {
        match self.items.get_mut(index) {
            Some(item) if !title.trim().is_empty() => {
                item.title = cap(title);
                true
            }
            _ => false,
        }
    }

    /** Remove one bookmark by its position. */
    pub fn remove(&mut self, index: usize) -> bool {
        (index < self.items.len()) && {
            self.items.remove(index);
            true
        }
    }

    /** Serialize after checking every bound a read would check. */
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        let valid = self.version == BOOKMARKS_VERSION
            && self.items.len() <= BOOKMARKS_LIMIT
            && self.items.iter().all(|item| {
                session_url_valid(&item.url)
                    && item.title.chars().count() <= super::history::TITLE_CHARS
            });
        if !valid {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > BOOKMARKS_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /** Read a saved record as untrusted input: strict fields, version, sizes and addresses. */
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > BOOKMARKS_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let bookmarks: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        bookmarks.encode()?;
        Ok(bookmarks)
    }
}
