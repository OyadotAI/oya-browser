/**
 * Manual hyphenation (CSS Text 3 `hyphens: manual`, the initial value, and `auto`, which Eye
 * treats as manual for want of dictionaries): a soft hyphen (U+00AD) is a break opportunity, and
 * a line that ends at one shows the `hyphenate-character` (Blink's U+2010 for `auto`) after it.
 * Without it a word full of soft hyphens overflowed its box as one unbreakable word.
 */
use super::flow::*;

/// The soft hyphen: invisible unless a line breaks after it.
pub(super) const SOFT_HYPHEN: char = '\u{ad}';
/// The hyphen `hyphenate-character: auto` shows (Blink picks U+2010 when the font has it).
const AUTO_HYPHEN: &str = "\u{2010}";

/** The string `node` shows at a hyphenated line end: its `hyphenate-character`, or U+2010. */
pub(super) fn hyphen(node: &Node) -> &str {
    node.style
        .hyphenate_character
        .as_deref()
        .unwrap_or(AUTO_HYPHEN)
}

/** Ends a line that broke at a soft wrap with the hyphen when its last text run stops at a soft
 * hyphen, widening that run by the hyphen's advance before the line is aligned. The run may
 * belong to any element on the line (`high&shy;<span>way</span>`). */
pub(super) fn mark(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    frags: &[(super::line::Frag, f64)],
) -> EngineResult<()> {
    let last = frags.iter().rev().find_map(|(frag, _)| match frag.piece {
        super::line::Piece::Text { item, line } => Some((item, line)),
        super::line::Piece::Atomic { .. } => None,
    });
    let Some((item, line)) = last else {
        return Ok(());
    };
    let ends = items
        .get(item)
        .and_then(|i| i.lines.get(line))
        .is_some_and(|run| run.text.ends_with(SOFT_HYPHEN));
    if !ends {
        return Ok(());
    }
    let node = context.node(items[item].local_id)?;
    if node.style.hyphens_none {
        return Ok(());
    }
    let hyphen = hyphen(node);
    let width = context.measure(hyphen, super::sizing::face_for(&node.style, Some(hyphen)))?;
    let run = &mut items[item].lines[line];
    run.text.push_str(hyphen);
    run.width += width;
    Ok(())
}

/** The unbreakable pieces of `word` min-content measures: the text between its break
 * opportunities under `breaks`, a piece ending at a soft hyphen with the hyphen it would show. */
pub(super) fn pieces(node: &Node, word: &str, breaks: super::wrap::Breaks) -> Vec<String> {
    super::wrap::segments(word, breaks)
        .into_iter()
        .map(
            |piece| match piece.ends_with(SOFT_HYPHEN) && !node.style.hyphens_none {
                true => format!("{piece}{}", hyphen(node)),
                false => piece.to_string(),
            },
        )
        .collect()
}
