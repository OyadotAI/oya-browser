/**
 * Persistence for the platform APIs through the profile's sealed record store: service worker
 * registrations, Cache Storage and stored credentials each own one record, replaced whole and
 * read back as untrusted input. Without a store (headless runs, tests) they live for the session
 * only; `--ephemeral` desktops pass an in-memory store, so nothing touches the Keychain.
 */
use crate::profile::{Record, RecordStore};
use serde::{Serialize, de::DeserializeOwned};
use std::sync::Arc;

/** The record store, when the runtime was given one. */
#[derive(Default)]
pub(in crate::runtime) struct Records {
    store: Option<Arc<dyn RecordStore>>,
}

impl Records {
    /** Use `store` from now on. */
    pub fn attach(&mut self, store: Arc<dyn RecordStore>) {
        self.store = Some(store);
    }

    /** Read one record; absent, unreadable or invalid records read as the default. */
    pub fn load<T: DeserializeOwned + Default>(&self, record: Record) -> T {
        self.store
            .as_ref()
            .and_then(|store| store.load(record).ok().flatten())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    /** Replace one record; false when it is too large or the store refused it. */
    pub fn save<T: Serialize>(&self, record: Record, value: &T) -> bool {
        let Some(store) = &self.store else {
            return true;
        };
        match serde_json::to_vec(value) {
            Ok(bytes) if bytes.len() <= record.limit() => store.replace(record, &bytes).is_ok(),
            _ => false,
        }
    }

    /** Whether an encoding of `value` fits the record's bound. */
    pub fn fits<T: Serialize>(record: Record, value: &T) -> bool {
        serde_json::to_vec(value).is_ok_and(|bytes| bytes.len() <= record.limit())
    }
}

impl crate::runtime::supervisor::Runtime {
    /** Give the platform APIs the profile's record store and restore what it holds. */
    pub fn attach_platform_records(&mut self, store: Arc<dyn RecordStore>) {
        self.platform.records.attach(store);
        self.platform.credentials = self.platform.records.load(Record::Credentials);
        self.platform.caches = self.platform.records.load(Record::CacheStorage);
        self.platform.workers = self.platform.records.load(Record::ServiceWorkers);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::test_support::MemoryRecordStore;

    /** A saved value reads back; a corrupt record reads as the default. */
    #[test]
    fn records_round_trip_and_reject_corruption() {
        let store = Arc::new(MemoryRecordStore::default());
        let mut records = Records::default();
        records.attach(store.clone());
        assert!(records.save(Record::Credentials, &vec![1, 2, 3]));
        let back: Vec<i32> = records.load(Record::Credentials);
        assert_eq!(back, vec![1, 2, 3]);
        store.seed(Record::Credentials, b"not json".to_vec());
        let back: Vec<i32> = records.load(Record::Credentials);
        assert!(back.is_empty());
    }
}
