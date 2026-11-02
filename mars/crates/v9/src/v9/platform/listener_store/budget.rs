/** Listener storage is bounded by retained native and worst-case encoded bytes, not a tag count. */
use super::{Listener, ListenerKey};
use crate::protocol::{EngineError, EngineResult};

/** Native registrations may occupy one quarter of the unchanged total encoded VM-state ceiling. */
pub(in crate::v9) const STORAGE_BYTES: usize = crate::constants::VM_STATE_BYTES / 4;
/** Fixed JSON fields, maximal integer identities, callback/signal references and punctuation. */
const WIRE_RECORD_BYTES: usize = 256;
/** BTree nodes reserve inline key/value slots; a factor of three covers ordinary node slack. */
const TREE_SLACK: usize = 3;
/** Amortized parent/child pointers and allocator metadata beyond the inline key/value slots. */
const TREE_POINTERS: usize = 8;
/** A root with only one entry can still allocate a whole BTree node; reserve it independently. */
const TREE_ROOT_BYTES: usize = 4096;
/** Worst-case JSON escaping emits six ASCII bytes for one original control byte. */
const ESCAPED_BYTE: usize = 6;
/** Charge the larger fixed representation, without assuming one platform's Rust enum layout. */
const NATIVE_RECORD_BYTES: usize = TREE_SLACK
    * (std::mem::size_of::<Listener>() + std::mem::size_of::<ListenerKey>())
    + TREE_POINTERS * std::mem::size_of::<usize>();
/** The fixed charge covers both independently bounded representations. */
const RECORD_BYTES: usize = if NATIVE_RECORD_BYTES > WIRE_RECORD_BYTES {
    NATIVE_RECORD_BYTES
} else {
    WIRE_RECORD_BYTES
};

/** Event kinds exist in both the index and record; real capacities cover native spare allocation. */
pub(super) fn record(kind: &str, kind_capacity: usize, key_capacity: usize) -> usize {
    RECORD_BYTES.saturating_add(
        kind.len()
            .saturating_mul(ESCAPED_BYTE)
            .max(kind_capacity.saturating_add(key_capacity)),
    )
}

/** One root allowance plus all record costs is the same bound at admission and restoration. */
pub(super) fn validate(bytes: usize) -> EngineResult<()> {
    if bytes.saturating_add(TREE_ROOT_BYTES) > STORAGE_BYTES {
        return Err(EngineError::limit(
            "V9 event listener storage bytes",
            STORAGE_BYTES,
        ));
    }
    Ok(())
}
