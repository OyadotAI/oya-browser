/** Borrowed UTF-16 views: code-point iteration keeps unpaired surrogates intact. */
use super::JsString;
use crate::{
    constants::{
        UTF16_HIGH_SURROGATE_END, UTF16_HIGH_SURROGATE_START, UTF16_LOW_SURROGATE_END,
        UTF16_LOW_SURROGATE_START, VM_ARRAY_LENGTH,
    },
    protocol::{EngineError, EngineResult},
    v9::source::whitespace,
};
impl JsString {
    /** Iterate UTF-16 units without requiring a contiguous backing allocation. */
    pub fn code_units(&self) -> impl Iterator<Item = u16> + '_ {
        let flat = self.flat_units();
        (0..self.len()).map(move |index| match flat {
            Some(units) => units[index],
            None => self.unit_at(index).expect("valid string index"),
        })
    }
    /** Borrow code-point slices only when their contiguous backing fits the string bound. */
    pub fn try_code_points(&self) -> EngineResult<CodePointCursor<'_>> {
        Ok(CodePointCursor {
            units: self.try_as_units()?,
            offset: 0,
        })
    }
    /** Default string iteration borrows code-point slices while preserving unmatched surrogates. */
    pub fn code_points(&self) -> CodePointCursor<'_> {
        CodePointCursor {
            units: self.as_units(),
            offset: 0,
        }
    }
    /// Borrow raw code units for indexed reads without decoding or replacing surrogates.
    pub fn as_units(&self) -> &[u16] {
        self.0.units()
    }
    /// Only ECMAScript edge whitespace is removed; surrogate and interior units remain unchanged.
    pub fn trimmed(&self) -> EngineResult<Self> {
        let start = (0..self.len())
            .position(|index| !space(self.unit_at(index).expect("valid string index")))
            .unwrap_or(self.len());
        let end = (start..self.len())
            .rfind(|index| !space(self.unit_at(*index).expect("valid string index")))
            .map_or(start, |index| index + 1);
        self.slice_range(start, end)
    }
    /// Literal splitting follows requested limits while rejecting only an actual excess output field.
    pub fn split_literal(&self, separator: Option<&Self>, limit: u32) -> EngineResult<Vec<Self>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let Some(separator) = separator else {
            return Ok(vec![self.clone()]);
        };
        if separator.is_empty() {
            return self.split_units(limit);
        }
        self.split_matches(separator, limit)
    }
    /** Empty separators split code units rather than Unicode scalar values or graphemes. */
    fn split_units(&self, limit: u32) -> EngineResult<Vec<Self>> {
        let count = self.len().min(limit as usize);
        if count > VM_ARRAY_LENGTH {
            return Err(split_limit());
        }
        (0..count)
            .map(|index| self.slice_range(index, index + 1))
            .collect()
    }
    /** Prefix fallback keeps exact literal matching linear even for repeated near-matching text. */
    fn split_matches(&self, separator: &Self, limit: u32) -> EngineResult<Vec<Self>> {
        let separator = separator.try_as_units()?;
        let prefix = prefix_lengths(separator);
        let (mut parts, mut matched, mut start) = (Vec::new(), 0, 0);
        for index in 0..self.len() {
            matched = next_match(
                separator,
                &prefix,
                matched,
                self.unit_at(index).expect("valid string index"),
            );
            if matched != separator.len() {
                continue;
            }
            push_part(&mut parts, self, start, index + 1 - separator.len())?;
            if parts.len() == limit as usize {
                return Ok(parts);
            }
            start = index + 1;
            matched = 0;
        }
        push_part(&mut parts, self, start, self.len())?;
        Ok(parts)
    }
}
/** A monotonic borrowed cursor performs no allocation or conversion at the VM string boundary. */
pub struct CodePointCursor<'a> {
    /// The owned string outlives every returned one- or two-unit slice.
    units: &'a [u16],
    /// Exhaustion never restarts iteration or rescans preceding code units.
    offset: usize,
}
impl<'a> Iterator for CodePointCursor<'a> {
    type Item = &'a [u16];
    /** Only an adjacent high/low pair advances by two; every other unit survives unchanged. */
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.offset;
        let first = *self.units.get(start)?;
        let paired = (UTF16_HIGH_SURROGATE_START..=UTF16_HIGH_SURROGATE_END).contains(&first)
            && self.units.get(start + 1).is_some_and(|unit| {
                (UTF16_LOW_SURROGATE_START..=UTF16_LOW_SURROGATE_END).contains(unit)
            });
        self.offset += if paired { 2 } else { 1 };
        Some(&self.units[start..self.offset])
    }
}

/** Surrogate units are never ECMAScript whitespace and remain valid owned JavaScript text. */
fn space(unit: u16) -> bool {
    char::from_u32(u32::from(unit)).is_some_and(whitespace)
}
/** The prefix table stores only bounded indices and never constructs intermediate substrings. */
fn prefix_lengths(pattern: &[u16]) -> Vec<usize> {
    let mut prefix = vec![0; pattern.len()];
    for index in 1..pattern.len() {
        prefix[index] = next_match(pattern, &prefix, prefix[index - 1], pattern[index]);
    }
    prefix
}
/** Falling back through prior prefixes avoids rescanning already-matched units. */
fn next_match(pattern: &[u16], prefix: &[usize], mut matched: usize, unit: u16) -> usize {
    while matched > 0 && pattern[matched] != unit {
        matched = prefix[matched - 1];
    }
    if pattern[matched] == unit {
        matched += 1;
    }
    matched
}
/** Budget failure happens before copying or allocating an excess output field. */
fn push_part(
    parts: &mut Vec<JsString>,
    text: &JsString,
    start: usize,
    end: usize,
) -> EngineResult<()> {
    if parts.len() >= VM_ARRAY_LENGTH {
        return Err(split_limit());
    }
    parts.push(text.slice_range(start, end)?);
    Ok(())
}
/** Literal output and heap arrays share the same named element cap. */
fn split_limit() -> EngineError {
    EngineError::limit("V9 split output elements", VM_ARRAY_LENGTH)
}
