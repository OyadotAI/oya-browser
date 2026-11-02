/**
 * `::first-letter` boxes: a block with a first-letter style hands it down to the first text of
 * its first formatted line, which splits its leading letter (with the punctuation around it)
 * into a box of that style, inline, floated, or as an `initial-letter`.
 */
use super::flow::*;
use crate::document::{ComputedStyle, style::Float};
use std::borrow::Cow;

/// The pending first letter a block found when it opened, restored when it closes.
pub(super) struct Scope {
    /// The pending letter before this block.
    saved: Option<(usize, Box<ComputedStyle>)>,
    /// This block passed the outer letter on to its content, so whatever its content left of
    /// it (consumed or still pending) stands after it.
    inherited: bool,
}

/** Opens a block container's first-letter scope: its own `::first-letter` style, else the
 * pending one of the block around it, which a new formatting context (a float, an inline-block,
 * a table cell) never receives. */
pub(super) fn open(context: &mut LayoutContext<'_>, node: &Node, fresh: bool) -> Scope {
    let saved = context.first_letter.take();
    let own = node.style.first_letter.clone().map(|s| (node.local_id, s));
    let inherited = own.is_none() && !fresh;
    context.first_letter = if inherited { saved.clone() } else { own };
    Scope { saved, inherited }
}

/** Closes a block's scope: a letter the block set or kept from its content ends with it. */
pub(super) fn close(context: &mut LayoutContext<'_>, scope: Scope) {
    if !scope.inherited {
        context.first_letter = scope.saved;
    }
}

/**
 * The element's own text to lay out after placing a pending first letter from its start, as
 * Blink's `FirstLetterPseudoElement` splits the first text node. Text met after content already
 * on the line cancels the letter; whitespace-only text leaves it pending for the next text.
 */
pub(super) fn lead<'n>(
    context: &mut LayoutContext<'_>,
    node: &'n Node,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<Cow<'n, str>> {
    let own = super::text::own_text(node);
    if context.first_letter.is_none() {
        return Ok(own);
    }
    if !flow.frags.is_empty() || !flow.finished.is_empty() {
        context.first_letter = None;
        return Ok(own);
    }
    let Some(range) = letter_range(&own) else {
        if !own.trim().is_empty() {
            context.first_letter = None;
        }
        return Ok(own);
    };
    let Some((owner, mut style)) = context.first_letter.take() else {
        return Ok(own);
    };
    if node.local_id != owner {
        inherit_from_text(&mut style, &context.node(owner)?.style, &node.style);
    }
    let letter = synthetic(node, owner, *style, &own[range.clone()]);
    place(context, &letter, owner, flow, items)?;
    Ok(Cow::Owned(own[range.end..].to_string()))
}

/** A letter inside a descendant inherits from the text's own element, as Blink parents the
 * pseudo-element there: each font and color value the letter's style merely inherited from the
 * block (equal to the block's) takes the text's instead. */
fn inherit_from_text(letter: &mut ComputedStyle, block: &ComputedStyle, text: &ComputedStyle) {
    if letter.color == block.color {
        letter.color.clone_from(&text.color);
    }
    if letter.font_size == block.font_size {
        letter.font_size = text.font_size;
    }
    if (letter.bold, letter.font_weight) == (block.bold, block.font_weight) {
        (letter.bold, letter.font_weight) = (text.bold, text.font_weight);
    }
    if letter.font_style == block.font_style {
        letter.font_style = text.font_style;
    }
    if letter.font_families == block.font_families {
        letter.font_family = text.font_family;
        letter.font_families.clone_from(&text.font_families);
    }
}

/** Lays the letter box out: as an initial letter, a float, or an inline box on the line. */
fn place(
    context: &mut LayoutContext<'_>,
    letter: &Node,
    owner: usize,
    flow: &mut Flow,
    items: &mut Vec<DisplayItem>,
) -> EngineResult<()> {
    if let Some(initial) = letter.style.initial_letter {
        let paragraph = context.node(owner)?.style.clone();
        return super::initial_letter::place(context, letter, &paragraph, initial, (flow, items));
    }
    if letter.style.float != Float::None {
        let letter = fitted(context, letter)?;
        return super::float::float(context, &letter, flow, items);
    }
    node_layout(context, letter, flow, items, None)
}

/** A floated letter with an auto width takes its text's advance: its identity is its block's,
 * whose cached intrinsic widths would otherwise size it. */
pub(super) fn fitted(context: &mut LayoutContext<'_>, letter: &Node) -> EngineResult<Node> {
    let mut letter = letter.clone();
    if letter.style.width.is_none() {
        let face = super::sizing::face_for(&letter.style, Some(&letter.text));
        letter.style.width = Some(context.measure(&letter.text, face)?);
    }
    Ok(letter)
}

/** A stand-in node carrying the letter under its block's identity (so painters find the
 * block's `::first-letter` style), with no spaces of its own around it. */
fn synthetic(node: &Node, owner: usize, style: ComputedStyle, text: &str) -> Node {
    let mut letter = node.clone();
    letter.local_id = owner;
    letter.tag = FIRST_LETTER.into();
    letter.text = text.into();
    letter.leading_space.clear();
    letter.space_leading = false;
    letter.space_after = false;
    letter.href = None;
    letter.image_src = None;
    letter.table_cell = None;
    letter.style = style.into();
    // A vertical flow lays out in a rotated copy whose styles are horizontal; a letter keeping
    // the vertical mode would root a rotated layout of its block's identity again, forever.
    letter.style.vertical_writing = node.style.vertical_writing;
    letter.style.writing_lr = node.style.writing_lr;
    letter.style.generated.clear();
    letter.style.first_letter = None;
    letter.style.first_line = None;
    letter
}

/// The tag of a first-letter box; its `::` prefix marks it generated, so it has no children.
pub(super) const FIRST_LETTER: &str = "::first-letter";

/**
 * Byte range of the first letter in `text`, per Blink's `FirstLetterLength`: leading collapsible
 * spaces are skipped, then any punctuation, one letter (a typographic unit with its combining
 * marks), and the punctuation right after it. None when the text is blank, is punctuation only,
 * puts a space between the punctuation and the letter, starts with a non-collapsible space (no
 * letter is styled after `&nbsp;` or `&emsp;`, as Chromium renders first-letter-space-not-
 * selected), or starts in a cursive script, whose letter split from its word would lose its
 * joined form.
 */
pub(super) fn letter_range(text: &str) -> Option<std::ops::Range<usize>> {
    let start = text.len()
        - text
            .trim_start_matches(|c: char| c.is_ascii_whitespace())
            .len();
    let mut chars = text[start..].char_indices().map(|(at, c)| (start + at, c));
    let mut current = chars.next()?;
    while punctuation(current.1) {
        current = chars.next()?;
    }
    if current.1.is_whitespace() || joining(current.1) {
        return None;
    }
    let letter_end = current.0 + current.1.len_utf8();
    let mut end = letter_end;
    for (at, c) in chars {
        // A flag is two regional indicators: the second joins the letter right after it.
        let paired = regional(current.1) && regional(c) && at == letter_end;
        if !(combining(c) || punctuation(c) || paired) {
            break;
        }
        end = at + c.len_utf8();
    }
    Some(start..end)
}

/** Unicode open, close, initial, final and other punctuation (Ps, Pe, Pi, Pf, Po), for the
 * scripts the punctuation tables below cover; dashes and connectors are not included. */
fn punctuation(c: char) -> bool {
    PUNCTUATION
        .iter()
        .any(|(low, high)| (*low..=*high).contains(&c))
}

/// Ranges of Ps, Pe, Pi, Pf and Po punctuation in ASCII, Latin-1, General Punctuation and CJK.
// ponytail: a hand-picked subset of the Unicode punctuation categories; a full table would
// cover other scripts' punctuation when a page needs it.
const PUNCTUATION: [(char, char); 17] = [
    ('!', '#'),
    ('%', '*'),
    (',', ','),
    ('.', '/'),
    (':', ';'),
    ('?', '@'),
    ('[', ']'),
    ('{', '{'),
    ('}', '}'),
    ('\u{a1}', '\u{a1}'),
    ('\u{a7}', '\u{a7}'),
    ('\u{ab}', '\u{ab}'),
    ('\u{b6}', '\u{b7}'),
    ('\u{bb}', '\u{bb}'),
    ('\u{bf}', '\u{bf}'),
    ('\u{2016}', '\u{205e}'),
    ('\u{3001}', '\u{3011}'),
];

/** Letters of the Arabic, Syriac and N'Ko scripts, which join their neighbors. */
// ponytail: Mars shapes each box's text alone; split letters of other joining scripts
// (Mongolian, Hanifi Rohingya) also lose their forms until shaping crosses boxes.
fn joining(c: char) -> bool {
    matches!(c, '\u{600}'..='\u{7ff}' | '\u{860}'..='\u{8ff}' | '\u{fb50}'..='\u{fdff}' | '\u{fe70}'..='\u{feff}')
}

/** Regional indicator symbols, two of which make one flag. */
fn regional(c: char) -> bool {
    ('\u{1f1e6}'..='\u{1f1ff}').contains(&c)
}

/** Combining marks that extend the letter before them into one typographic unit. */
fn combining(c: char) -> bool {
    matches!(c, '\u{300}'..='\u{36f}' | '\u{1ab0}'..='\u{1aff}' | '\u{20d0}'..='\u{20ff}' | '\u{fe20}'..='\u{fe2f}')
}
