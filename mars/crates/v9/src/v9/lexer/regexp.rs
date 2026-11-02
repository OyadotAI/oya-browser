/** RegExp literal flags are early grammar errors, independently of evaluating or compiling the pattern. */
use super::{Source, errors, identifier_continue};
use crate::protocol::{EngineResult, SourceLocation};

/** Unicode and Unicode-sets occupy one slot because these modes are mutually exclusive. */
const FLAG_SLOTS: &str = "dgimsuy";
/** Literal flag errors have their own diagnostic instead of becoming following identifier tokens. */
const INVALID_FLAGS: &str = "Invalid regular expression flags";

/** Preserve raw valid flag order while rejecting escapes, unknown parts, repetitions and mixed Unicode modes. */
pub(super) fn flags(source: &mut Source<'_>, start: SourceLocation) -> EngineResult<String> {
    let beginning = source.rest();
    let mut seen = 0_u8;
    while let Some(character) = source
        .peek()
        .filter(|character| *character == '\\' || identifier_continue(*character))
    {
        let bit = flag_bit(character).ok_or_else(|| errors::syntax(start, INVALID_FLAGS))?;
        if seen & bit != 0 {
            return Err(errors::syntax(start, INVALID_FLAGS));
        }
        seen |= bit;
        source.next();
    }
    Ok(beginning[..beginning.len() - source.rest().len()].to_string())
}

/** Valid flags are ASCII; both Unicode spellings intentionally share their exclusivity bit. */
fn flag_bit(character: char) -> Option<u8> {
    let character = if character == 'v' { 'u' } else { character };
    FLAG_SLOTS.find(character).map(|index| 1_u8 << index)
}
