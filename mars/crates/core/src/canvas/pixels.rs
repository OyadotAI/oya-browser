/** Shared pixel ownership bounds process allocations and makes candidate copies transactional. */
use crate::protocol::{EngineError, EngineResult};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error, SeqAccess, Visitor},
};
use std::{
    fmt,
    ops::Deref,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// The stricter coordinator ceiling also bounds V9; snapshots share immutable allocations.
const PROCESS_BYTES: usize = 96 * 1024 * 1024;
/// One maximum-size canonical RGBA surface.
const BITMAP_BYTES: usize = super::EDGE as usize * super::EDGE as usize * super::CHANNELS;
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

/** A reservation is released even when allocation, deserialization or drawing fails. */
#[derive(Debug)]
pub struct Reservation(usize);
impl Reservation {
    /** Reserve before allocation; concurrent service work cannot exceed the process ceiling. */
    pub fn new(bytes: usize) -> EngineResult<Self> {
        ALLOCATED
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                n.checked_add(bytes).filter(|n| *n <= PROCESS_BYTES)
            })
            .map_err(|_| {
                EngineError::invalid(
                    "QuotaExceededError: Canvas process allocation limit exceeded.",
                )
            })?;
        Ok(Self(bytes))
    }
}
impl Drop for Reservation {
    /** Every last snapshot releases the storage it actually owns. */
    fn drop(&mut self) {
        ALLOCATED.fetch_sub(self.0, Ordering::AcqRel);
    }
}
#[derive(Debug)]
struct Storage {
    bytes: Vec<u8>,
    _reservation: Reservation,
}
/// Cloning a candidate shares immutable bytes; the first write explicitly reserves its own copy.
#[derive(Clone, Debug)]
pub struct Pixels(Arc<Storage>);
impl Default for Pixels {
    fn default() -> Self {
        Self(Arc::new(Storage {
            bytes: Vec::new(),
            _reservation: Reservation(0),
        }))
    }
}
impl PartialEq for Pixels {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.0.bytes == other.0.bytes
    }
}
impl Deref for Pixels {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.0.bytes
    }
}
impl Pixels {
    /// Allocate a zeroed backing only after both per-bitmap and process admission.
    pub fn zeroed(len: usize) -> EngineResult<Self> {
        if len > BITMAP_BYTES {
            return Err(EngineError::limit("Canvas bitmap bytes", BITMAP_BYTES));
        }
        let reservation = Reservation::new(len)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(len)
            .map_err(|_| EngineError::limit("Canvas allocation", len))?;
        bytes.resize(len, 0);
        Ok(Self(Arc::new(Storage {
            bytes,
            _reservation: reservation,
        })))
    }
    /// Copy-on-write is fallible and charged before mutation, preserving committed candidates.
    pub fn make_mut(&mut self) -> EngineResult<&mut [u8]> {
        if Arc::get_mut(&mut self.0).is_none() {
            let mut copy = Self::zeroed(self.len())?;
            Arc::get_mut(&mut copy.0)
                .expect("new storage")
                .bytes
                .copy_from_slice(self);
            *self = copy;
        }
        Ok(&mut Arc::get_mut(&mut self.0).expect("unique pixels").bytes)
    }
}
impl Serialize for Pixels {
    /** Keep the existing transparent byte-array wire form. */
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.bytes.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Pixels {
    /** Refuse oversized arrays during decoding, before constructing a forged bitmap. */
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Bounded;
        impl<'de> Visitor<'de> for Bounded {
            type Value = Pixels;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("bounded canvas RGBA bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Pixels, A::Error> {
                if seq.size_hint().is_some_and(|n| n > BITMAP_BYTES) {
                    return Err(A::Error::custom("Canvas bitmap too large"));
                }
                let mut reservation =
                    Reservation::new(BITMAP_BYTES).map_err(|e| A::Error::custom(e.message))?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(BITMAP_BYTES)
                    .map_err(A::Error::custom)?;
                while let Some(byte) = seq.next_element::<u8>()? {
                    if bytes.len() == BITMAP_BYTES {
                        return Err(A::Error::custom("Canvas bitmap too large"));
                    }
                    bytes.push(byte);
                }
                bytes.shrink_to_fit();
                let capacity = bytes.capacity();
                ALLOCATED.fetch_sub(reservation.0 - capacity, Ordering::AcqRel);
                reservation.0 = capacity;
                Ok(Pixels(Arc::new(Storage {
                    bytes,
                    _reservation: reservation,
                })))
            }
        }
        d.deserialize_seq(Bounded)
    }
}
