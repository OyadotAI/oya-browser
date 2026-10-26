/** Immutable function-name snapshots compress bound prefixes while retaining delayed wrapped-name resolution depth. */
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{CONFIGURABLE, DEFAULT_ATTRS};
use serde::{Deserialize, Serialize};

/** This is function metadata, not a general string rope; the tail is always an already captured string. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct FunctionName {
    /** Each bind contributes exactly this fixed prefix without duplicating the captured tail. */
    pub prefixes: u32,
    /** GetName iterates binds but crosses a stack-checked recursive boundary at each wrapped function. */
    pub wrapped: u32,
    /** A value field lets existing retained-string accounting deduplicate shared tails. */
    pub tail: JsValue,
    /** A virtual own data descriptor retains author changes to its attributes. */
    pub attrs: u8,
}

/** UTF-16 code units in the specification's fixed bound-function prefix. */
pub(in crate::v9) const BOUND_PREFIX: &[u16] = &[98, 111, 117, 110, 100, 32];

impl FunctionName {
    /** Real Gets have already captured this string; later target mutations cannot alter it. */
    pub(in crate::v9) fn flat(tail: JsValue) -> Self {
        Self {
            prefixes: 0,
            wrapped: 0,
            tail,
            attrs: CONFIGURABLE,
        }
    }
    /** Snapshot copies are values; descriptor attributes always reset on a fresh function. */
    pub(in crate::v9) fn value_snapshot(&self) -> Self {
        let mut value = self.clone();
        value.attrs = CONFIGURABLE;
        value
    }
    /** Counter bounds reflect actual retained function records, rather than permitting forged unbounded recipes. */
    pub(in crate::v9) fn valid(&self, functions: usize) -> bool {
        matches!(self.tail, JsValue::String(_))
            && self.attrs & !DEFAULT_ATTRS == 0
            && self.prefixes as usize + self.wrapped as usize <= functions
    }
    /** Logical length can be checked without allocating the repeated prefixes. */
    pub(in crate::v9) fn length(&self) -> Option<usize> {
        let JsValue::String(tail) = &self.tail else {
            return None;
        };
        (self.prefixes as usize)
            .checked_mul(BOUND_PREFIX.len())?
            .checked_add(tail.len())
    }
}
