/** JSON source units can come from borrowed host text or an immutable compact VM string. */
use super::JsString;
use crate::protocol::EngineResult;

/** Source access remains indexed so diagnostics and reviver spans share the original positions. */
#[derive(Clone, Copy)]
pub(super) enum Input<'a> {
    Units(&'a [u16]),
    String(&'a JsString),
}

impl Input<'_> {
    /** Reuse already contiguous storage while leaving compact sources unmaterialized. */
    pub(super) fn from_string(text: &JsString) -> Input<'_> {
        match text.flat_units() {
            Some(units) => Input::Units(units),
            None => Input::String(text),
        }
    }
    /** Obtain one UTF-16 unit without flattening compact VM strings. */
    pub(super) fn get(self, index: usize) -> Option<u16> {
        match self {
            Self::Units(units) => units.get(index).copied(),
            Self::String(text) => text.unit_at(index),
        }
    }
    /** Source offsets count UTF-16 units in both representations. */
    pub(super) fn len(self) -> usize {
        match self {
            Self::Units(units) => units.len(),
            Self::String(text) => text.len(),
        }
    }
    /** Materialize only the requested bounded span where parser results need owned text. */
    pub(super) fn range(self, start: usize, end: usize) -> Vec<u16> {
        (start..end)
            .map(|index| self.get(index).expect("valid JSON source range"))
            .collect()
    }
    /** Preserve compact string storage for reviver source spans when possible. */
    pub(super) fn string_range(self, start: usize, end: usize) -> EngineResult<JsString> {
        match self {
            Self::Units(units) => JsString::from_slice(&units[start..end]),
            Self::String(text) => text.slice_range(start, end),
        }
    }
}
