/**
 * What crosses between a context's IndexedDB engine and the coordinator, and the coordinator's
 * per-origin database images. Names and keys travel as their JSON text and values as the
 * engine's serialized JSON, so every string here is well-formed UTF-8 even when a script's names
 * hold lone surrogates; the coordinator never interprets a value. A commit applies atomically:
 * every operation lands, or (past a bound) none does.
 */
use crate::constants::JS_STRING_UNITS;
use crate::protocol::{EngineError, EngineResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/** HostRequest kind of an IndexedDB coordinator request. */
pub const INDEXED_DB_KIND: &str = "indexeddb";
/** HostRequest method that loads one database image; the body is the database name's JSON text. */
pub const INDEXED_DB_OPEN: &str = "OPEN";
/** HostRequest method that lists the origin's databases. */
pub const INDEXED_DB_LIST: &str = "LIST";
/** Stored key and value bytes one database may hold; the engine's DATABASE_BYTES must match, and past it a commit fails with QuotaExceededError. */
pub const INDEXED_DB_DATABASE_BYTES: usize = 4 * 1024 * 1024;
/** A database image is one JavaScript string when it reaches the engine, so it is bounded by the longest string. */
pub const INDEXED_DB_RESPONSE_BYTES: usize = JS_STRING_UNITS;
/** Commit bytes one execution may record: four full-size images. */
pub const INDEXED_DB_EFFECT_BYTES: usize = 4 * JS_STRING_UNITS;
/** Stored bytes one origin may hold across its databases, which is also its sealed profile record's bound. */
pub const INDEXED_DB_ORIGIN_BYTES: usize = 8 * INDEXED_DB_DATABASE_BYTES;
/** A sealed origin record's bound: JSON may escape each stored character into six. */
pub const INDEXED_DB_ORIGIN_RECORD_BYTES: usize = 6 * INDEXED_DB_ORIGIN_BYTES;
/* A full image answers within the bytes one resume step's responses may carry, so the frame budget for responses still holds. */
const _: () = assert!(
    INDEXED_DB_RESPONSE_BYTES
        <= crate::constants::SCRIPT_REQUESTS_PER_STEP * crate::constants::VM_RESPONSE_BYTES
);
/** Operations one commit may carry. */
const OPERATIONS: usize = 1 << 20;
/** Fixed accounting per operation and record, beside its strings. */
const OVERHEAD: usize = 16;

/** One committed transaction for one database. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedDbCommit {
    /// The database name's JSON text.
    pub database: String,
    /// The changes, applied in order.
    pub ops: Vec<IndexedDbOperation>,
}

/** One change a commit makes. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum IndexedDbOperation {
    /// An upgrade's whole new image: version, stores and records.
    Replace {
        version: u64,
        stores: BTreeMap<String, IndexedDbStore>,
    },
    /// Store one record (key and value as JSON text).
    Put {
        store: String,
        key: String,
        value: String,
    },
    /// Remove one record.
    Delete { store: String, key: String },
    /// Remove every record of a store.
    Clear { store: String },
    /// Replace a store's metadata (its key generator moved).
    Meta { store: String, meta: String },
    /// Delete the database.
    Drop {},
}

/** One object store: opaque metadata JSON and records by key JSON. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedDbStore {
    /// Key path, key generator and index definitions, as the engine wrote them.
    pub meta: String,
    /// Serialized values by key JSON.
    pub records: BTreeMap<String, String>,
}

/** One database image. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedDbDatabase {
    /// The database version; 0 never persists.
    pub version: u64,
    /// Stores by name JSON.
    pub stores: BTreeMap<String, IndexedDbStore>,
}

/** Every database of one origin in one storage partition. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IndexedDbOrigin {
    /// Databases by name JSON.
    pub databases: BTreeMap<String, IndexedDbDatabase>,
}

impl IndexedDbStore {
    /** Accounted bytes: metadata, keys and values. */
    fn bytes(&self) -> usize {
        self.records
            .iter()
            .fold(self.meta.len() + OVERHEAD, |total, (key, value)| {
                total + key.len() + value.len() + OVERHEAD
            })
    }
}

impl IndexedDbOperation {
    /** Accounted bytes of one operation. */
    fn bytes(&self) -> usize {
        OVERHEAD
            + match self {
                Self::Replace { stores, .. } => stores
                    .iter()
                    .map(|(name, store)| name.len() + store.bytes())
                    .sum(),
                Self::Put { store, key, value } => store.len() + key.len() + value.len(),
                Self::Delete { store, key } => store.len() + key.len(),
                Self::Clear { store } => store.len(),
                Self::Meta { store, meta } => store.len() + meta.len(),
                Self::Drop {} => 0,
            }
    }
}

impl IndexedDbCommit {
    /** Accounted bytes of the whole commit. */
    pub fn bytes(&self) -> usize {
        self.ops
            .iter()
            .fold(self.database.len() + OVERHEAD, |total, op| {
                total + op.bytes()
            })
    }

    /** Reject commits no engine would produce: too many operations or more than one execution may carry. */
    pub fn validate(&self) -> EngineResult<()> {
        if self.ops.len() > OPERATIONS || self.bytes() > INDEXED_DB_EFFECT_BYTES {
            return Err(EngineError::limit(
                "IndexedDB commit bytes",
                INDEXED_DB_EFFECT_BYTES,
            ));
        }
        Ok(())
    }
}

impl IndexedDbDatabase {
    /** Apply one record-level operation; changes to a store this image lacks (another page dropped it) are ignored. */
    fn apply(&mut self, operation: &IndexedDbOperation) {
        match operation {
            IndexedDbOperation::Put { store, key, value } => {
                if let Some(store) = self.stores.get_mut(store) {
                    store.records.insert(key.clone(), value.clone());
                }
            }
            IndexedDbOperation::Delete { store, key } => {
                if let Some(store) = self.stores.get_mut(store) {
                    store.records.remove(key);
                }
            }
            IndexedDbOperation::Clear { store } => {
                if let Some(store) = self.stores.get_mut(store) {
                    store.records.clear();
                }
            }
            IndexedDbOperation::Meta { store, meta } => {
                if let Some(store) = self.stores.get_mut(store) {
                    store.meta.clone_from(meta);
                }
            }
            IndexedDbOperation::Replace { .. } | IndexedDbOperation::Drop {} => {}
        }
    }
}

impl IndexedDbOrigin {
    /** Accounted bytes of every database. */
    pub fn bytes(&self) -> usize {
        self.databases
            .iter()
            .map(|(name, database)| {
                name.len()
                    + database
                        .stores
                        .values()
                        .map(IndexedDbStore::bytes)
                        .sum::<usize>()
            })
            .sum()
    }

    /** Apply a commit atomically: on a bound violation nothing changes. Returns whether anything changed. */
    pub fn apply(&mut self, commit: &IndexedDbCommit) -> EngineResult<bool> {
        commit.validate()?;
        let mut next = self.databases.get(&commit.database).cloned();
        for operation in &commit.ops {
            match operation {
                IndexedDbOperation::Replace { version, stores } => {
                    next = Some(IndexedDbDatabase {
                        version: *version,
                        stores: stores.clone(),
                    });
                }
                IndexedDbOperation::Drop {} => next = None,
                record => {
                    if let Some(database) = next.as_mut() {
                        database.apply(record);
                    }
                }
            }
        }
        let previous = match next {
            Some(database) => self.databases.insert(commit.database.clone(), database),
            None => self.databases.remove(&commit.database),
        };
        if self.bytes() > INDEXED_DB_ORIGIN_BYTES {
            match previous {
                Some(database) => self.databases.insert(commit.database.clone(), database),
                None => self.databases.remove(&commit.database),
            };
            return Err(EngineError::limit(
                "IndexedDB origin bytes",
                INDEXED_DB_ORIGIN_BYTES,
            ));
        }
        Ok(true)
    }

    /** The image the engine loads for one database name JSON: the database, or `null`. */
    pub fn image(&self, database: &str) -> EngineResult<String> {
        serde_json::to_string(&self.databases.get(database))
            .map_err(|_| EngineError::invalid("Invalid IndexedDB image."))
    }

    /** The `[[nameJson, version], ...]` listing databases() reads. */
    pub fn listing(&self) -> EngineResult<String> {
        let entries: Vec<(&String, u64)> = self
            .databases
            .iter()
            .map(|(name, database)| (name, database.version))
            .collect();
        serde_json::to_string(&entries).map_err(|_| EngineError::invalid("Invalid IndexedDB list."))
    }

    /** The bytes of a sealed profile record. */
    pub fn encode(&self) -> EngineResult<Vec<u8>> {
        let bytes = serde_json::to_vec(self)
            .map_err(|_| EngineError::invalid("Invalid IndexedDB record."))?;
        if bytes.len() > INDEXED_DB_ORIGIN_RECORD_BYTES {
            return Err(EngineError::limit(
                "IndexedDB record bytes",
                INDEXED_DB_ORIGIN_RECORD_BYTES,
            ));
        }
        Ok(bytes)
    }

    /** Read a profile record back as untrusted input. */
    pub fn decode(bytes: &[u8]) -> EngineResult<Self> {
        let origin: Self = serde_json::from_slice(bytes)
            .map_err(|_| EngineError::invalid("Invalid IndexedDB record."))?;
        if origin.bytes() > INDEXED_DB_ORIGIN_BYTES {
            return Err(EngineError::invalid("Invalid IndexedDB record."));
        }
        Ok(origin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /** A put commit for one record. */
    fn put(key: &str, value: &str) -> IndexedDbCommit {
        IndexedDbCommit {
            database: "\"db\"".into(),
            ops: vec![IndexedDbOperation::Put {
                store: "\"s\"".into(),
                key: key.into(),
                value: value.into(),
            }],
        }
    }

    /** An upgrade image with one empty store. */
    fn created() -> IndexedDbCommit {
        let mut stores = BTreeMap::new();
        stores.insert("\"s\"".to_string(), IndexedDbStore::default());
        IndexedDbCommit {
            database: "\"db\"".into(),
            ops: vec![IndexedDbOperation::Replace { version: 1, stores }],
        }
    }

    /** Record operations land in the image a replace created; a drop removes the database. */
    #[test]
    fn commits_apply_in_order_and_drop_removes_the_database() {
        let mut origin = IndexedDbOrigin::default();
        origin.apply(&created()).unwrap();
        origin.apply(&put("k", "1")).unwrap();
        assert!(origin.image("\"db\"").unwrap().contains("\"k\":\"1\""));
        assert_eq!(origin.listing().unwrap(), "[[\"\\\"db\\\"\",1]]");
        let drop = IndexedDbCommit {
            database: "\"db\"".into(),
            ops: vec![IndexedDbOperation::Drop {}],
        };
        origin.apply(&drop).unwrap();
        assert_eq!(origin.image("\"db\"").unwrap(), "null");
    }

    /** A commit that would push the origin past its bound changes nothing. */
    #[test]
    fn an_oversized_commit_leaves_the_origin_unchanged() {
        let mut origin = IndexedDbOrigin::default();
        origin.apply(&created()).unwrap();
        let big = "x".repeat(INDEXED_DB_DATABASE_BYTES);
        for index in 0..(INDEXED_DB_ORIGIN_BYTES / INDEXED_DB_DATABASE_BYTES) - 1 {
            origin.apply(&put(&index.to_string(), &big)).unwrap();
        }
        let before = origin.clone();
        assert!(origin.apply(&put("last", &big)).is_err());
        assert_eq!(origin, before);
    }

    /** The wire format is the engine's: tagged operations, unknown fields refused. */
    #[test]
    fn commits_parse_the_engine_wire_format_strictly() {
        let text = r#"{"database":"\"db\"","ops":[{"op":"clear","store":"\"s\""},{"op":"drop"}]}"#;
        let commit: IndexedDbCommit = serde_json::from_str(text).unwrap();
        assert_eq!(commit.ops.len(), 2);
        assert!(
            serde_json::from_str::<IndexedDbCommit>(
                r#"{"database":"x","ops":[{"op":"drop","extra":1}]}"#
            )
            .is_err()
        );
    }
}
