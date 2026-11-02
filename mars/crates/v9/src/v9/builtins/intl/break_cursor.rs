/**
 * The cursor of a conformance `Intl.v8BreakIterator`: the pure state moves behind its steps. The
 * boundaries are computed once by `load`, so `advance`, `rewind` and `position` are index moves
 * with no rescans. ICU's conventions are kept: DONE is -1, and `breakType` names the segment the
 * cursor last passed.
 */
use super::segmentation::Piece;
use crate::v9::vm::data::{BreakIteratorData, BreakType};

/** What `current` and `next` return once the iterator has passed the last boundary (ICU's DONE). */
pub(super) const DONE: f64 = -1.0;
/** Hiragana and katakana, the code points `breakType` reports as kana. */
const KANA: std::ops::RangeInclusive<char> = '\u{3040}'..='\u{30FF}';
/** The CJK unified ideographs, the code points `breakType` reports as ideo. */
const IDEOGRAPHS: std::ops::RangeInclusive<char> = '\u{4E00}'..='\u{9FFF}';
/** The `breakType` reported before the first step and after the last boundary. */
const NO_TYPE: &str = "none";
/** The `breakType` reported once the iterator has run off the end. */
const UNKNOWN_TYPE: &str = "unknown";

/** The type of one segment: only word-like text has one, and it is the first character's script
or, for digits, a number. */
fn classify(text: &str, wordy: bool) -> BreakType {
    if !wordy {
        return BreakType::None;
    }
    if text.chars().all(char::is_numeric) {
        return BreakType::Number;
    }
    match text.chars().next() {
        Some(first) if KANA.contains(&first) => BreakType::Kana,
        Some(first) if IDEOGRAPHS.contains(&first) => BreakType::Ideo,
        _ => BreakType::Letter,
    }
}

/** Stores the boundaries and segment types of `pieces`, with the cursor on the first boundary. */
pub(super) fn load(state: &mut BreakIteratorData, pieces: &[Piece], total: usize) {
    state.boundaries = pieces
        .iter()
        .map(|piece| piece.start)
        .chain(std::iter::once(total))
        .collect();
    state.break_types = pieces
        .iter()
        .map(|piece| classify(&piece.text, piece.wordy))
        .collect();
    state.cursor = 0;
}

/** Rewinds the cursor to the first boundary and returns it, for `first()`. */
pub(super) fn rewind(state: &mut BreakIteratorData) -> f64 {
    state.cursor = 0;
    position(state)
}

/** Steps the cursor forward; running off the end parks it past the last boundary. */
pub(super) fn advance(state: &mut BreakIteratorData) -> f64 {
    if state.cursor + 1 < state.boundaries.len() {
        state.cursor += 1;
    } else {
        state.cursor = state.boundaries.len();
    }
    position(state)
}

/** The boundary under the cursor, or DONE once the cursor has passed the last one. */
pub(super) fn position(state: &BreakIteratorData) -> f64 {
    state
        .boundaries
        .get(state.cursor)
        .map_or(DONE, |at| *at as f64)
}

/** The name of the segment type the cursor sits after. */
pub(super) fn type_name(state: &BreakIteratorData) -> &'static str {
    match state.cursor {
        0 => NO_TYPE,
        at => state
            .break_types
            .get(at - 1)
            .map_or(UNKNOWN_TYPE, |kind| match kind {
                BreakType::None => NO_TYPE,
                BreakType::Number => "number",
                BreakType::Letter => "letter",
                BreakType::Kana => "kana",
                BreakType::Ideo => "ideo",
            }),
    }
}
