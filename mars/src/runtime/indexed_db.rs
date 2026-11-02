/**
 * The coordinator's side of IndexedDB. Each storage partition (top-level site plus origin) owns
 * one `IndexedDbOrigin`: every database image of that origin there. Pages and workers load an
 * image when they open a database and send back the records each transaction changed when it
 * commits; the coordinator applies commits to the image and writes the origin's sealed record
 * through the profile store. Authority (origin and partition) always comes from the native
 * context that ran the script, never from the request. An ephemeral profile has no store, so
 * its databases live only as long as the browser runs.
 * ponytail: the whole origin record is rewritten per commit; split it per database once origins
 * keep many megabytes and commit often.
 */
use super::supervisor::Runtime;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{
    HostRequest, HostResponse, INDEXED_DB_LIST, INDEXED_DB_OPEN, IndexedDbCommit, IndexedDbOrigin,
};
use std::collections::{BTreeMap, BTreeSet};

/** Profile record names are this prefix plus the partition key's SHA-256 in hex. */
const RECORD_PREFIX: &str = "indexeddb-";
/** HTTP status of a successful answer. */
const OK: u16 = 200;

/** Every loaded origin image, plus the partitions whose saved record could not be read (never overwritten). */
#[derive(Default)]
pub(super) struct IndexedDbStores {
    origins: BTreeMap<String, IndexedDbOrigin>,
    unreadable: BTreeSet<String>,
}

/** The partition key: the top-level site and the origin, or an error for contexts without web storage (opaque origins). */
pub(super) fn partition_key(origin: &str, top_level_url: &str) -> EngineResult<String> {
    let url = url::Url::parse(top_level_url)
        .map_err(|_| EngineError::invalid("Invalid IndexedDB partition."))?;
    let site = crate::network::partition_site(&url)
        .ok_or_else(|| EngineError::unsupported("IndexedDB requires a web partition."))?;
    let parsed =
        url::Url::parse(origin).map_err(|_| EngineError::invalid("Invalid IndexedDB origin."))?;
    if origin == "null" || parsed.origin().ascii_serialization() != origin {
        return Err(EngineError::unsupported(
            "IndexedDB requires a tuple origin.",
        ));
    }
    Ok(format!("{site}\n{origin}"))
}

/** The opaque profile record name for a partition key. */
fn record_name(key: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, key.as_bytes());
    let hex: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{RECORD_PREFIX}{hex}")
}

/** A successful answer carrying `body`. */
fn answered(request: &HostRequest, body: String) -> HostResponse {
    HostResponse {
        id: request.id,
        status: OK,
        url: request.url.clone(),
        body,
        ..HostResponse::default()
    }
}

/** A failed answer; the engine reports UnknownError. */
fn failed(request: &HostRequest, error: &EngineError) -> HostResponse {
    HostResponse {
        id: request.id,
        url: request.url.clone(),
        error: Some(error.message.clone()),
        ..HostResponse::default()
    }
}

impl Runtime {
    /** The origin image for a partition, loading its saved record the first time. */
    fn indexed_db_origin(&mut self, key: &str) -> EngineResult<&mut IndexedDbOrigin> {
        if self.indexed_db.unreadable.contains(key) {
            return Err(EngineError::unsupported(
                "IndexedDB storage is unavailable.",
            ));
        }
        if !self.indexed_db.origins.contains_key(key) {
            let loaded = match &self.storage_store {
                Some(store) => match store.load_indexed_db(&record_name(key)) {
                    Ok(Some(bytes)) => IndexedDbOrigin::decode(&bytes).unwrap_or_default(),
                    Ok(None) => IndexedDbOrigin::default(),
                    Err(error) => {
                        self.indexed_db.unreadable.insert(key.into());
                        return Err(error.engine_error());
                    }
                },
                None => IndexedDbOrigin::default(),
            };
            self.indexed_db.origins.insert(key.into(), loaded);
        }
        Ok(self.indexed_db.origins.get_mut(key).expect("loaded above"))
    }

    /** Answer one engine request for the context at `origin` under `top_level_url`. */
    pub(super) fn answer_indexed_db(
        &mut self,
        origin: &str,
        top_level_url: &str,
        request: &HostRequest,
    ) -> HostResponse {
        let answer = partition_key(origin, top_level_url).and_then(|key| {
            let images = self.indexed_db_origin(&key)?;
            match request.method.as_str() {
                INDEXED_DB_OPEN => images.image(request.body.as_deref().unwrap_or_default()),
                INDEXED_DB_LIST => images.listing(),
                _ => Err(EngineError::invalid("Invalid IndexedDB request.")),
            }
        });
        match answer {
            Ok(body) => answered(request, body),
            Err(error) => failed(request, &error),
        }
    }

    /** Apply a context's committed transactions in order, then save the origin. A refused commit is reported and leaves the image as it was. */
    pub(super) fn commit_indexed_db(
        &mut self,
        origin: &str,
        top_level_url: &str,
        commits: &[IndexedDbCommit],
    ) {
        if commits.is_empty() {
            return;
        }
        let Ok(key) = partition_key(origin, top_level_url) else {
            return;
        };
        let applied = self.indexed_db_origin(&key).map(|images| {
            commits
                .iter()
                .filter(|commit| images.apply(commit).is_err())
                .count()
        });
        match applied {
            Ok(0) => {}
            Ok(refused) => eprintln!(
                "Mars IndexedDB: {refused} commit(s) exceeded the origin's storage bound and were not saved."
            ),
            Err(_) => return,
        }
        self.save_indexed_db(&key);
    }

    /** Write one partition's record through the profile store; ephemeral profiles keep it in memory. */
    fn save_indexed_db(&mut self, key: &str) {
        let (Some(store), Some(images)) = (&self.storage_store, self.indexed_db.origins.get(key))
        else {
            return;
        };
        let saved = images.encode().and_then(|bytes| {
            store
                .replace_indexed_db(&record_name(key), &bytes)
                .map_err(|error| error.engine_error())
        });
        if saved.is_err() {
            eprintln!("Mars IndexedDB: a database commit could not be saved to the profile.");
        }
    }

    /** Forget every database, saved and loaded, as clearing saved site storage does. */
    pub(super) fn clear_indexed_db(&mut self) -> EngineResult<()> {
        if let Some(store) = &self.storage_store {
            store
                .clear_indexed_db()
                .map_err(|error| error.engine_error())?;
        }
        self.indexed_db = IndexedDbStores::default();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Partition keys pair the top-level site with the exact origin, so a third-party frame is kept apart. */
    #[test]
    fn partition_keys_separate_top_level_sites() {
        let first = partition_key("https://cdn.example.net", "https://a.com/page").unwrap();
        let second = partition_key("https://cdn.example.net", "https://b.com/").unwrap();
        assert_ne!(first, second);
        assert!(partition_key("null", "https://a.com/").is_err());
    }

    /** Record names are opaque digests, never the origin text. */
    #[test]
    fn record_names_hide_the_origin() {
        let name = record_name("https://a.com\nhttps://a.com");
        assert!(name.starts_with(RECORD_PREFIX));
        assert!(!name.contains("a.com"));
        assert_eq!(name.len(), RECORD_PREFIX.len() + 64);
    }
}
