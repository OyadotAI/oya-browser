/**
 * Lenient mask subjects for a dropped selector. A subject that is itself `:is(...)` or
 * `:where(...)` masks each argument's subject instead of everything, so an unsupported
 * argument such as `thead + tbody` does not turn into a universal mask; each subject keeps the
 * number of ancestors its selector demands, which rules out the root and body wrappers.
 */
use super::{
    constants::{BLOCK_DEPTH, SELECTOR_ARGUMENTS},
    selector_parse::{self, Compound},
    syntax::{find_top, split_compounds, split_top},
};
/** Every superset subject with the ancestors it needs; empty when the selector matches nothing. */
pub(super) fn subjects(text: &str) -> Vec<(Compound, usize)> {
    let mut out = Vec::new();
    collect(text.trim(), 0, 0, &mut out);
    out
}
/** Adds the subjects of `text`, under `outer` ancestors required by enclosing selectors. */
fn collect(text: &str, outer: usize, depth: usize, out: &mut Vec<(Compound, usize)>) {
    let ancestors = outer + ancestors(text);
    let last = split_compounds(text);
    if let Some(arguments) =
        list(last).filter(|args| depth < BLOCK_DEPTH && args.len() <= SELECTOR_ARGUMENTS)
    {
        for argument in arguments {
            collect(argument.trim(), ancestors, depth + 1, out);
        }
        return;
    }
    if let Some(compound) = selector_parse::subject(last) {
        out.push((compound, ancestors));
    }
}
/** The arguments when a compound starts with `:is(` or `:where(`; later simples only narrow it. */
fn list(compound: &str) -> Option<Vec<&str>> {
    let lower = compound.get(..7).unwrap_or(compound).to_ascii_lowercase();
    let open = [":is(", ":where("]
        .iter()
        .find(|f| lower.starts_with(**f))?
        .len();
    let close = find_top(compound, open, b")");
    Some(split_top(compound.get(open..close)?, b','))
}
/** Descendant and child combinators before the subject: each needs one more ancestor. */
fn ancestors(text: &str) -> usize {
    let (mut pieces, mut siblings, mut index) = (0usize, 0usize, 0);
    while index <= text.len() {
        let end = find_top(text, index, b" \t\r\n>+~");
        if text
            .get(index..end)
            .is_some_and(|piece| !piece.trim().is_empty())
        {
            pieces += 1;
        }
        siblings += usize::from(matches!(text.as_bytes().get(end), Some(b'+' | b'~')));
        index = end + 1;
    }
    pieces.saturating_sub(1).saturating_sub(siblings)
}
