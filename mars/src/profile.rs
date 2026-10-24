/** Desktop profile records are sealed with one Keychain-held key; see `vault` for the format. */
use crate::{
    constants::{SESSION_PROFILE_BYTES, WEB_STORAGE_PROFILE_BYTES},
    protocol::EngineError,
};
pub use mars_core::profile::{ProfileError, validate_size};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(target_os = "macos")]
mod keychain;
pub mod vault;

/// One owner persists a complete candidate before publishing it to navigation.
pub trait ProfileStore: Send + Sync {
    /// Read the one default profile; absence means a new empty profile.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Atomically replace the stored profile.
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError>;
    /// Remove the stored profile; an absent item is already clear.
    fn clear(&self) -> Result<(), ProfileError>;
}

/** Web Storage uses a separate secure item so clearing cookies preserves site state. */
pub trait WebStorageStore: Send + Sync {
    /// Read the versioned origin map; absence means a new empty profile.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Replace one complete bounded origin map atomically.
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError>;
    /// Remove an absent or existing record when rolling back a first write.
    fn clear(&self) -> Result<(), ProfileError>;
    /// Read a separate fail-closed marker before any origin data is restored.
    fn recovery_required(&self) -> Result<bool, ProfileError>;
    /// Persist a marker that contains no origin, key, or value data.
    fn mark_recovery(&self) -> Result<(), ProfileError>;
    /// Remove the marker only after the origin map has been deleted successfully.
    fn clear_recovery(&self) -> Result<(), ProfileError>;
}

/** History, bookmarks and settings each own one independent secure record, so clearing one never touches another or the cookie jar. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Record {
    /// Visited pages, newest first.
    History,
    /// Saved pages.
    Bookmarks,
    /// Appearance, home page, downloads folder and bookmarks bar.
    Settings,
}

impl Record {
    /** The most bytes a valid record of this kind encodes to; larger reads are corrupt. */
    pub fn limit(self) -> usize {
        match self {
            Self::History => crate::runtime::history::HISTORY_BYTES,
            Self::Bookmarks => crate::runtime::bookmarks::BOOKMARKS_BYTES,
            Self::Settings => crate::runtime::settings::SETTINGS_BYTES,
        }
    }
}

/** One complete versioned record per kind, replaced whole, read back as untrusted bytes. */
pub trait RecordStore: Send + Sync {
    /// Read one record; absence means a fresh profile.
    fn load(&self, record: Record) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Replace one complete record.
    fn replace(&self, record: Record, bytes: &[u8]) -> Result<(), ProfileError>;
}

/** Session URLs use an independent secure record and never contain request bodies. */
pub trait SessionStore: Send + Sync {
    /// Read the last committed tab list; absence means a fresh browser session.
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError>;
    /// Replace the complete bounded HTTP(S) tab list.
    fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError>;
}

/** Ordered committed GET destinations are restored only after browser profiles are ready. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionProfile {
    /// Version rejects formats with incompatible restore semantics.
    pub version: u32,
    /// Persisted eligible URLs preserve order and never contain POST bodies.
    pub urls: Vec<String>,
    /// Selected URL index; an empty profile uses zero.
    pub active_index: usize,
}

impl SessionProfile {
    /// Validate URLs, selection, tab count and encoded size before persistence.
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        let valid = self.version == crate::constants::SESSION_PROFILE_VERSION
            && self.urls.len() <= crate::constants::TABS
            && (self.urls.is_empty() && self.active_index == 0
                || self.active_index < self.urls.len())
            && self.urls.iter().all(|value| session_url_valid(value));
        if !valid {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > SESSION_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /// Decode strict session state; malformed data is ignored by startup restore.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > SESSION_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let profile: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        profile.encode()?;
        Ok(profile)
    }
}

/// Session URLs must be credential-free absolute HTTP(S) destinations within protocol bounds.
pub(crate) fn session_url_valid(value: &str) -> bool {
    value.len() <= crate::constants::URL_BYTES
        && url::Url::parse(value).is_ok_and(|parsed| {
            ["http", "https"].contains(&parsed.scheme())
                && parsed.host_str().is_some()
                && parsed.username().is_empty()
                && parsed.password().is_none()
        })
}

/** Complete localStorage state is one independent sealed record, replaced whole. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct WebStorageProfile {
    /// Version rejects records whose semantics this runtime does not understand.
    pub version: u32,
    /// Canonical serialized origin to bounded string key/value map.
    pub origins: BTreeMap<String, BTreeMap<String, String>>,
}

impl WebStorageProfile {
    /// Serialize only after entry, value, aggregate and encoded bounds pass.
    pub fn encode(&self) -> Result<Vec<u8>, ProfileError> {
        if self.version != crate::constants::WEB_STORAGE_PROFILE_VERSION
            || self.origins.values().any(|area| {
                area.len() > crate::constants::STORAGE_ENTRIES
                    || area.iter().any(|(key, value)| {
                        key.len() > crate::constants::STORAGE_VALUE_BYTES
                            || value.len() > crate::constants::STORAGE_VALUE_BYTES
                            || key.encode_utf16().count() > crate::constants::JS_STRING_UNITS
                            || value.encode_utf16().count() > crate::constants::JS_STRING_UNITS
                    })
                    || area
                        .iter()
                        .map(|(key, value)| key.len().saturating_add(value.len()))
                        .sum::<usize>()
                        > crate::constants::STORAGE_AREA_BYTES
            })
        {
            return Err(ProfileError::Corrupt);
        }
        let bytes = serde_json::to_vec(self).map_err(|_| ProfileError::Corrupt)?;
        if bytes.len() > WEB_STORAGE_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        Ok(bytes)
    }

    /// Decode a strict bounded record without ever including stored values in errors.
    pub fn decode(bytes: &[u8]) -> Result<Self, ProfileError> {
        if bytes.len() > WEB_STORAGE_PROFILE_BYTES {
            return Err(ProfileError::Corrupt);
        }
        let profile: Self = serde_json::from_slice(bytes).map_err(|_| ProfileError::Corrupt)?;
        profile.encode()?;
        Ok(profile)
    }
}

/// In-memory stores: tests and `--ephemeral` runs use them so nothing touches the Keychain.
pub mod test_support {
    use super::{ProfileError, ProfileStore, SessionStore, WebStorageStore};
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    /// Isolated profile bytes allow restart and failure tests without the user Keychain.
    #[derive(Default)]
    pub struct MemoryStore {
        value: Mutex<Option<Vec<u8>>>,
        fail_replace: AtomicBool,
        fail_clear: AtomicBool,
        fail_load: AtomicBool,
    }

    impl MemoryStore {
        /// Seed an isolated test store without using the user's Keychain.
        pub fn seed(&self, bytes: Vec<u8>) {
            if let Ok(mut value) = self.value.lock() {
                *value = Some(bytes);
            }
        }
        /// Fail exactly the next replacement before changing the stored record.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
        /// Fail exactly the next clear while retaining the stored record.
        pub fn fail_next_clear(&self) {
            self.fail_clear.store(true, Ordering::SeqCst);
        }
        /// Fail exactly the next load to exercise inaccessible secure storage.
        pub fn fail_next_load(&self) {
            self.fail_load.store(true, Ordering::SeqCst);
        }
        /// Read test state for assertions without formatting the secret payload.
        pub fn has_record(&self) -> bool {
            self.value.lock().is_ok_and(|value| value.is_some())
        }
    }

    impl ProfileStore for MemoryStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            if self.fail_load.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = Some(bytes.to_vec()))
                .map_err(|_| ProfileError::Unavailable)
        }
        fn clear(&self) -> Result<(), ProfileError> {
            if self.fail_clear.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = None)
                .map_err(|_| ProfileError::Unavailable)
        }
    }

    /** Isolated secure-record stand-in proves localStorage restart and rollback semantics. */
    #[derive(Default)]
    pub struct MemoryWebStorageStore {
        value: Mutex<Option<Vec<u8>>>,
        recovery: Mutex<bool>,
        fail_replace: AtomicBool,
        fail_load: AtomicBool,
        fail_clear: AtomicBool,
        fail_mark: AtomicBool,
        fail_recovery_read: AtomicBool,
        fail_recovery_clear: AtomicBool,
    }

    /** Hermetic session repository supports restart and invalid-record tests. */
    #[derive(Default)]
    pub struct MemorySessionStore {
        value: Mutex<Option<Vec<u8>>>,
        fail_load: AtomicBool,
        fail_replace: AtomicBool,
    }

    impl MemorySessionStore {
        /// Seed an encoded record without exposing URL contents in diagnostics.
        pub fn seed(&self, bytes: Vec<u8>) {
            if let Ok(mut value) = self.value.lock() {
                *value = Some(bytes);
            }
        }
        /// Fail one load to verify startup remains usable without restoring.
        pub fn fail_next_load(&self) {
            self.fail_load.store(true, Ordering::SeqCst);
        }
        /// Fail one save; runtime browsing must remain available.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
        /// Return only presence for privacy-preserving assertions.
        pub fn has_record(&self) -> bool {
            self.value.lock().is_ok_and(|value| value.is_some())
        }
        /// Read the test record for structured assertions without formatting bytes.
        pub fn record(&self) -> Option<Vec<u8>> {
            self.value.lock().ok().and_then(|value| value.clone())
        }
    }

    impl SessionStore for MemorySessionStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            if self.fail_load.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = Some(bytes.to_vec()))
                .map_err(|_| ProfileError::Unavailable)
        }
    }

    /** Hermetic history, bookmarks and settings records for restart tests and verification windows. */
    #[derive(Default)]
    pub struct MemoryRecordStore {
        values: Mutex<std::collections::HashMap<super::Record, Vec<u8>>>,
        fail_replace: AtomicBool,
    }

    impl MemoryRecordStore {
        /// Seed one record, valid or not, as if a previous run saved it.
        pub fn seed(&self, record: super::Record, bytes: Vec<u8>) {
            if let Ok(mut values) = self.values.lock() {
                values.insert(record, bytes);
            }
        }
        /// Read one record for structured assertions.
        pub fn record(&self, record: super::Record) -> Option<Vec<u8>> {
            self.values.lock().ok()?.get(&record).cloned()
        }
        /// Fail one save; browsing must continue and the next save retries.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
    }

    impl super::RecordStore for MemoryRecordStore {
        fn load(&self, record: super::Record) -> Result<Option<Vec<u8>>, ProfileError> {
            let values = self.values.lock().map_err(|_| ProfileError::Unavailable)?;
            Ok(values.get(&record).cloned())
        }
        fn replace(&self, record: super::Record, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            let mut values = self.values.lock().map_err(|_| ProfileError::Unavailable)?;
            values.insert(record, bytes.to_vec());
            Ok(())
        }
    }

    impl MemoryWebStorageStore {
        /// Read only record presence for tests that must not print site values.
        pub fn has_record(&self) -> bool {
            self.value.lock().is_ok_and(|value| value.is_some())
        }
        /// Seed a strict malformed record without exposing it in assertion output.
        pub fn seed(&self, bytes: Vec<u8>) {
            if let Ok(mut value) = self.value.lock() {
                *value = Some(bytes);
            }
        }
        /// Fail one write without modifying the prior record.
        pub fn fail_next_replace(&self) {
            self.fail_replace.store(true, Ordering::SeqCst);
        }
        /// Fail one load to exercise secure profile recovery.
        pub fn fail_next_load(&self) {
            self.fail_load.store(true, Ordering::SeqCst);
        }
        /// Fail origin deletion while retaining both data and recovery state.
        pub fn fail_next_clear(&self) {
            self.fail_clear.store(true, Ordering::SeqCst);
        }
        /// Fail marker persistence to verify restart safety is reported honestly.
        pub fn fail_next_mark(&self) {
            self.fail_mark.store(true, Ordering::SeqCst);
        }
        /// Fail marker reads so startup must reject the origin map.
        pub fn fail_next_recovery_read(&self) {
            self.fail_recovery_read.store(true, Ordering::SeqCst);
        }
        /// Fail marker deletion after the origin map has already been removed.
        pub fn fail_next_recovery_clear(&self) {
            self.fail_recovery_clear.store(true, Ordering::SeqCst);
        }
        /// Seed recovery state without exposing any site value in test output.
        pub fn seed_recovery(&self) {
            if let Ok(mut value) = self.recovery.lock() {
                *value = true;
            }
        }
        /// Read marker presence without formatting stored site data.
        pub fn has_recovery(&self) -> bool {
            self.recovery.lock().is_ok_and(|value| *value)
        }
    }

    impl WebStorageStore for MemoryWebStorageStore {
        fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
            if self.fail_load.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|value| value.clone())
                .map_err(|_| ProfileError::Unavailable)
        }
        fn replace(&self, bytes: &[u8]) -> Result<(), ProfileError> {
            if self.fail_replace.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = Some(bytes.to_vec()))
                .map_err(|_| ProfileError::Unavailable)
        }
        fn clear(&self) -> Result<(), ProfileError> {
            if self.fail_clear.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.value
                .lock()
                .map(|mut value| *value = None)
                .map_err(|_| ProfileError::Unavailable)
        }
        fn recovery_required(&self) -> Result<bool, ProfileError> {
            if self.fail_recovery_read.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.recovery
                .lock()
                .map(|value| *value)
                .map_err(|_| ProfileError::Unavailable)
        }
        fn mark_recovery(&self) -> Result<(), ProfileError> {
            if self.fail_mark.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.recovery
                .lock()
                .map(|mut value| *value = true)
                .map_err(|_| ProfileError::Unavailable)
        }
        fn clear_recovery(&self) -> Result<(), ProfileError> {
            if self.fail_recovery_clear.swap(false, Ordering::SeqCst) {
                return Err(ProfileError::Unavailable);
            }
            self.recovery
                .lock()
                .map(|mut value| *value = false)
                .map_err(|_| ProfileError::Unavailable)
        }
    }
}

/// Convert storage failures to safe UI and protocol text without secret details.
pub fn unavailable() -> EngineError {
    ProfileError::Unavailable.engine_error()
}

/// Build the production cookie store on macOS without providing a plaintext fallback.
#[cfg(target_os = "macos")]
pub fn system_store() -> std::sync::Arc<dyn ProfileStore> {
    std::sync::Arc::new(vault::Cookies(keychain::system()))
}

/** Production localStorage is its own sealed record, independent of the cookie record. */
#[cfg(target_os = "macos")]
pub fn system_web_storage_store() -> std::sync::Arc<dyn WebStorageStore> {
    std::sync::Arc::new(vault::Storage(keychain::system()))
}

/** The desktop session is its own sealed record, independent of cookies and Web Storage. */
#[cfg(target_os = "macos")]
pub fn system_session_store() -> std::sync::Arc<dyn SessionStore> {
    std::sync::Arc::new(vault::Sessions(keychain::system()))
}

/** History, bookmarks and settings are sealed records, never plaintext files. */
#[cfg(target_os = "macos")]
pub fn system_record_store() -> std::sync::Arc<dyn RecordStore> {
    std::sync::Arc::new(vault::Records(keychain::system()))
}

/** Unsupported platforms keep history and bookmarks for the session only rather than in plaintext. */
#[cfg(not(target_os = "macos"))]
pub fn system_record_store() -> std::sync::Arc<dyn RecordStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/// Other desktop platforms cannot silently substitute weaker persistence.
#[cfg(not(target_os = "macos"))]
pub fn system_store() -> std::sync::Arc<dyn ProfileStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/** Unsupported platforms fail closed rather than persist browser state as plaintext. */
#[cfg(not(target_os = "macos"))]
pub fn system_web_storage_store() -> std::sync::Arc<dyn WebStorageStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/** Unsupported platforms skip tab persistence rather than writing URLs as plaintext. */
#[cfg(not(target_os = "macos"))]
pub fn system_session_store() -> std::sync::Arc<dyn SessionStore> {
    std::sync::Arc::new(UnsupportedStore)
}

/** Platforms without the current native desktop never persist cookies insecurely. */
#[cfg(not(target_os = "macos"))]
struct UnsupportedStore;
#[cfg(not(target_os = "macos"))]
impl ProfileStore for UnsupportedStore {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn clear(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}
#[cfg(not(target_os = "macos"))]
impl WebStorageStore for UnsupportedStore {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn clear(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn recovery_required(&self) -> Result<bool, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn mark_recovery(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn clear_recovery(&self) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}
#[cfg(not(target_os = "macos"))]
impl RecordStore for UnsupportedStore {
    fn load(&self, _record: Record) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _record: Record, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}
#[cfg(not(target_os = "macos"))]
impl SessionStore for UnsupportedStore {
    fn load(&self) -> Result<Option<Vec<u8>>, ProfileError> {
        Err(ProfileError::Unavailable)
    }
    fn replace(&self, _bytes: &[u8]) -> Result<(), ProfileError> {
        Err(ProfileError::Unavailable)
    }
}
