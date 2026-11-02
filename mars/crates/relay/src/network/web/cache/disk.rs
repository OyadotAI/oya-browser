/** The optional on-disk cache store: one bounded file per URL, under a directory the app owns. */
use super::{Key, Stored};
use crate::constants::{HEADER_BYTES, HTML_BYTES};
use crate::network::constants::{CACHE_DISK_BYTES, CACHE_DISK_ENTRIES};
use crate::protocol::{EngineError, EngineResult};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Mutex;

/** Without a directory the cache is memory only. */
static DIRECTORY: Mutex<Option<PathBuf>> = Mutex::new(None);

/** Entry file suffix; nothing else in the directory is read or removed. */
const SUFFIX: &str = "entry";

/** Bytes of the little-endian metadata length prefix. */
const PREFIX: usize = 4;

/** Largest file read back: a full body plus generous room for headers and Vary values. */
const FILE_BYTES: usize = HTML_BYTES + 4 * HEADER_BYTES;

/// Persists cache entries under `path` (created if missing), for example the profile's cache
/// folder. Until this is called, or if it fails, Relay caches in memory only.
pub fn set_cache_directory(path: PathBuf) -> EngineResult<()> {
    fs::create_dir_all(&path).map_err(|_| unavailable())?;
    let mut directory = DIRECTORY
        .lock()
        .map_err(|_| EngineError::service("network", false))?;
    *directory = Some(path);
    Ok(())
}

/**
 * Reads the entry for `key`. Files are treated as untrusted: oversized, truncated or
 * unparsable files, or one stored under another key (a hash collision), are removed and missed.
 */
pub(super) fn load(key: &Key) -> Option<Stored> {
    let path = path(key)?;
    if fs::metadata(&path).ok()?.len() > FILE_BYTES as u64 {
        let _ = fs::remove_file(&path);
        return None;
    }
    let bytes = fs::read(&path).ok()?;
    let stored = parse(&bytes).filter(|stored| stored.key == *key);
    if stored.is_none() {
        let _ = fs::remove_file(&path);
    }
    stored
}

/**
 * Writes atomically (temporary file, then rename) and trims the directory. Entries that vary
 * on Cookie stay in memory, so cookie values never reach the disk cache.
 */
pub(super) fn save(stored: &Stored) {
    let Some(path) = path(&stored.key) else {
        return;
    };
    if stored.vary.iter().any(|(name, _)| name == "cookie") {
        return;
    }
    let Ok(meta) = serde_json::to_vec(stored) else {
        return;
    };
    let Ok(length) = u32::try_from(meta.len()) else {
        return;
    };
    let mut bytes = length.to_le_bytes().to_vec();
    bytes.extend(meta);
    bytes.extend_from_slice(&stored.body);
    let temporary = path.with_extension("partial");
    if fs::write(&temporary, bytes).is_ok() && fs::rename(&temporary, &path).is_ok() {
        trim();
    } else {
        let _ = fs::remove_file(&temporary);
    }
}

/** Removes the entry for `key`, if any. */
pub(super) fn remove(key: &Key) {
    if let Some(path) = path(key) {
        let _ = fs::remove_file(path);
    }
}

/** Removes every entry file (and any half-written one); other files in the directory stay. */
pub(super) fn clear() {
    let Some(directory) = DIRECTORY
        .lock()
        .ok()
        .and_then(|directory| directory.clone())
    else {
        return;
    };
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    for entry in listing.filter_map(Result::ok) {
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|ext| ext == SUFFIX || ext == "partial")
        {
            let _ = fs::remove_file(path);
        }
    }
}

/** Metadata length, metadata JSON, then the raw body. */
fn parse(bytes: &[u8]) -> Option<Stored> {
    let length = u32::from_le_bytes(bytes.get(..PREFIX)?.try_into().ok()?) as usize;
    let meta = bytes.get(PREFIX..PREFIX.checked_add(length)?)?;
    let mut stored: Stored = serde_json::from_slice(meta).ok()?;
    stored.body = bytes[PREFIX + length..].to_vec();
    (stored.body.len() <= HTML_BYTES).then_some(stored)
}

/** A stable file name per key; the key inside the file settles collisions. */
fn path(key: &Key) -> Option<PathBuf> {
    let directory = DIRECTORY.lock().ok()?.clone()?;
    let mut hasher = std::hash::DefaultHasher::new();
    key.hash(&mut hasher);
    Some(directory.join(format!("{:016x}.{SUFFIX}", hasher.finish())))
}

/**
 * Removes the oldest entry files until both the count and byte budgets hold.
 * ponytail: scans the directory on every write, bounded by `CACHE_DISK_ENTRIES`; keep a
 * running index if writes ever show up in profiles.
 */
fn trim() {
    let Some(directory) = DIRECTORY
        .lock()
        .ok()
        .and_then(|directory| directory.clone())
    else {
        return;
    };
    let Ok(listing) = fs::read_dir(directory) else {
        return;
    };
    let mut files: Vec<_> = listing
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == SUFFIX))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), entry.path()))
        })
        .collect();
    files.sort();
    let mut total: u64 = files.iter().map(|(_, length, _)| length).sum();
    let mut count = files.len();
    for (_, length, path) in files {
        if count <= CACHE_DISK_ENTRIES && total <= CACHE_DISK_BYTES {
            break;
        }
        let _ = fs::remove_file(path);
        total -= length;
        count -= 1;
    }
}

/** The directory could not be created; the cache keeps working in memory. */
fn unavailable() -> EngineError {
    EngineError::new(
        "cache_unavailable",
        "The HTTP cache directory could not be created.",
        "Check the profile folder's permissions; pages still load without a disk cache.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::net::{IpAddr, Ipv4Addr};

    /** A small entry under a unique URL. */
    fn entry(index: usize, vary: &[(&str, &str)]) -> Stored {
        Stored {
            key: Key {
                partition: "https://a.example".into(),
                url: format!("https://a.example/{index}"),
                fixture: false,
                image: false,
                origin: None,
                no_avif: false,
            },
            headers: BTreeMap::from([("etag".into(), "\"x\"".into())]),
            body: b"body".to_vec(),
            vary: vary
                .iter()
                .map(|(n, v)| ((*n).into(), (*v).into()))
                .collect(),
            address: IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1)),
            stored_at: 0,
            initial_age: 0,
            lifetime: 0,
            no_cache: false,
        }
    }

    /** Round trip, cookie refusal, corruption recovery and the entry bound, in one ordered test because the directory is process-wide. */
    #[test]
    fn disk_entries_round_trip_and_stay_bounded() {
        let directory =
            std::env::temp_dir().join(format!("mars-relay-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        set_cache_directory(directory.clone()).expect("directory");
        let stored = entry(0, &[]);
        save(&stored);
        let loaded = load(&stored.key).expect("saved entry loads");
        assert_eq!(
            (loaded.body, loaded.headers),
            (stored.body.clone(), stored.headers.clone())
        );

        let cookie = entry(1, &[("cookie", "session=secret")]);
        save(&cookie);
        assert!(
            load(&cookie.key).is_none(),
            "cookie values never reach disk"
        );

        let file = path(&stored.key).expect("path");
        fs::write(&file, b"\xff\xff\xff\xffgarbage").expect("corrupt");
        assert!(load(&stored.key).is_none());
        assert!(!file.exists(), "corrupt files are removed");

        for index in 0..CACHE_DISK_ENTRIES + 3 {
            save(&entry(index + 2, &[]));
        }
        let count = fs::read_dir(&directory).expect("list").count();
        assert!(count <= CACHE_DISK_ENTRIES, "{count} files");
        *DIRECTORY.lock().expect("lock") = None;
        let _ = fs::remove_dir_all(&directory);
    }
}
