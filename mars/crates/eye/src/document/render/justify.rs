/**
 * `text-align: justify` and `text-align-last` after Blink's InlineLayoutAlgorithm::ApplyJustify:
 * a justified line spreads its free space evenly over its expansion opportunities (word
 * separators, or every character under `text-justify: inter-character`), and a line with none
 * aligns at its start. A run that gains space is split at its opportunities into runs placed
 * where the widened glyphs fall, so painters keep drawing each run as one shaped string.
 */
use super::flow::*;
use super::line::{Align, Frag, Piece};

/** Which lines of a block align how: lines ended by a soft wrap, and the rest (the last line
 * and lines before a forced break, which `text-align-last` governs). */
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Aligned {
    pub lines: Align,
    pub last: Align,
}

/** How a justified line spreads: `right` when its start is the right edge (the fallback when it
 * has no opportunity). Where each run's opportunities are follows its own `text-justify`. */
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Spread {
    pub right: bool,
}

impl Spread {
    /** The start alignment a justified line without opportunities falls back to. */
    pub fn start(self) -> Align {
        if self.right {
            Align::Right
        } else {
            Align::Left
        }
    }
}

/** A block's alignments: `text-align` for wrapped lines; `text-align-last` (or, when `auto`,
 * `text-align` with `justify` falling back to start) for the others. `text-justify: none`
 * turns justification into start alignment. */
pub(super) fn aligned(style: &crate::document::ComputedStyle) -> Aligned {
    let lines = keyword(&style.align, style);
    let last = match style.align_last.as_str() {
        "" if style.align == "justify" => keyword("start", style),
        "" => lines,
        value => keyword(value, style),
    };
    Aligned { lines, last }
}

/** One `text-align` keyword resolved against the base direction. */
fn keyword(value: &str, style: &crate::document::ComputedStyle) -> Align {
    let spread = Spread { right: style.rtl };
    match (value, style.rtl) {
        ("justify", _) if style.text_justify == "none" => spread.start(),
        ("justify", _) => Align::Justify(spread),
        ("center" | "webkit-center", _) => Align::Center,
        ("right", _) | ("start", true) | ("end", false) => Align::Right,
        _ => Align::Left,
    }
}

/** A line's fragment in visual order: its left edge and what it moves. */
type Placed = (f64, Piece);

/**
 * Spreads `free` over the line's opportunities, moving each fragment right by the space gained
 * before it and splitting its runs. False, with nothing moved, when the line has none.
 */
pub(super) fn spread(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    frags: &[(Frag, f64)],
    free: f64,
) -> EngineResult<bool> {
    let placed = ordered(items, frags);
    let last = placed
        .iter()
        .rposition(|(_, p)| matches!(p, Piece::Text { .. }));
    // A tab advances to a fixed stop, so only what follows the line's last tab can spread.
    let after_tab = placed
        .iter()
        .rposition(|(_, p)| is_tab(items, *p))
        .map_or(0, |at| at + 1);
    let mut marks = Vec::with_capacity(placed.len());
    for (index, (_, piece)) in placed.iter().enumerate() {
        let mode = mode(context, items, *piece);
        marks.push(match index < after_tab {
            true => Vec::new(),
            false => opportunities(items, *piece, mode, Some(index) == last),
        });
    }
    let count: usize = marks.iter().map(Vec::len).sum();
    if count == 0 || free <= 0.0 {
        return Ok(false);
    }
    let extra = free / count as f64;
    let mut gained = 0.0;
    for ((_, piece), marks) in placed.into_iter().zip(marks) {
        match piece {
            Piece::Text { item, line } => {
                split(context, items, (item, line), &marks, (gained, extra))?
            }
            Piece::Atomic { .. } => shift(items, piece, gained),
        }
        gained += extra * marks.len() as f64;
    }
    Ok(true)
}

/** The line's fragments sorted by their left edge, the order space accumulates in. */
fn ordered(items: &[DisplayItem], frags: &[(Frag, f64)]) -> Vec<Placed> {
    let mut placed: Vec<Placed> = frags
        .iter()
        .filter_map(|(frag, _)| {
            let left = match frag.piece {
                Piece::Text { item, line } => items.get(item)?.lines.get(line)?.x,
                Piece::Atomic { start, .. } => items.get(start)?.x,
            };
            Some((left, frag.piece))
        })
        .collect();
    placed.sort_by(|a, b| a.0.total_cmp(&b.0));
    placed
}

/** Where a run's own `text-justify` puts its opportunities, as Blink reads it per text item:
 * none, after word separators (`auto`, `inter-word`), or after every character. */
#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    None,
    Words,
    /** `auto`: word separators, and after each ideograph. */
    Auto,
    Chars,
}

/** The `text-justify` of the element a text piece belongs to. */
fn mode(context: &LayoutContext<'_>, items: &[DisplayItem], piece: Piece) -> Mode {
    let Piece::Text { item, .. } = piece else {
        return Mode::None;
    };
    let node = items.get(item).and_then(|i| context.node(i.local_id).ok());
    match node.map(|n| n.style.text_justify.as_str()) {
        Some("none") => Mode::None,
        Some("inter-character") => Mode::Chars,
        Some("inter-word") => Mode::Words,
        _ => Mode::Auto,
    }
}

/** Byte offsets just past each opportunity in a text piece; an atomic has none. The line's last
 * run keeps its trailing ones out, as Blink never expands at a line's end. */
fn opportunities(items: &[DisplayItem], piece: Piece, mode: Mode, last: bool) -> Vec<usize> {
    if mode == Mode::None {
        return Vec::new();
    }
    let Piece::Text { item, line } = piece else {
        return Vec::new();
    };
    let Some(text) = items
        .get(item)
        .and_then(|i| i.lines.get(line))
        .map(|l| &l.text)
    else {
        return Vec::new();
    };
    let body = if last {
        text.trim_end_matches(separator)
    } else {
        text
    };
    // `auto` also spreads after each ideograph, as Blink expands CJK text between its
    // characters (ShapeResultSpacing's ideograph opportunities).
    let mut marks: Vec<usize> = body
        .char_indices()
        .filter(|(_, c)| {
            mode == Mode::Chars
                || separator(*c)
                || (mode == Mode::Auto && crate::platform::autospace::ideograph(*c))
        })
        .map(|(at, c)| at + c.len_utf8())
        .collect();
    if last {
        marks.retain(|at| *at < body.len() || separator_ended(body));
    }
    marks
}

/** Whether `body` ends in a word separator, whose opportunity stays even at the line's end
 * (the trailing ones were already trimmed off the last run). */
fn separator_ended(body: &str) -> bool {
    body.chars().last().is_some_and(separator)
}

/** A tab: the empty run as wide as its advance that text layout places for each one. */
fn is_tab(items: &[DisplayItem], piece: Piece) -> bool {
    let Piece::Text { item, line } = piece else {
        return false;
    };
    items
        .get(item)
        .and_then(|i| i.lines.get(line))
        .is_some_and(|run| run.text.is_empty() && run.width > 0.0)
}

/** CSS Text's word-separator characters: the space, the no-break space, and the Ethiopic,
 * Aegean, Ugaritic and Phoenician word separators. */
const WORD_SEPARATORS: [char; 7] = [
    ' ',
    '\u{a0}',
    '\u{1361}',
    '\u{10100}',
    '\u{10101}',
    '\u{1039f}',
    '\u{1091f}',
];

/** A word separator, where inter-word justification expands. */
fn separator(c: char) -> bool {
    WORD_SEPARATORS.contains(&c)
}

/** A character `word-spacing` widened when layout measured the run. */
fn widened(c: char) -> bool {
    c == ' ' || c == '\u{a0}'
}

/** Moves an atomic fragment right by `dx`. */
fn shift(items: &mut [DisplayItem], piece: Piece, dx: f64) {
    if let Piece::Atomic { start, end } = piece {
        for item in items.iter_mut().take(end).skip(start) {
            super::line::translate(item, dx, 0.0);
        }
    }
}

/**
 * Splits run `line` of `item` after each of its `marks`: the first piece stays in place of the
 * run and the others are appended to the item's lines, each at the run's start plus the shaped
 * width of the text before it, `gained` from earlier fragments and `extra` per opportunity
 * before it. Each piece's width takes its own opportunity's share, so the pieces tile the line.
 */
fn split(
    context: &mut LayoutContext<'_>,
    items: &mut [DisplayItem],
    (item, line): (usize, usize),
    marks: &[usize],
    (gained, extra): (f64, f64),
) -> EngineResult<()> {
    let Some(run) = items.get(item).and_then(|i| i.lines.get(line)).cloned() else {
        return Ok(());
    };
    let starts = prefixes(context, &items[item], &run, marks)?;
    let mut pieces = Vec::with_capacity(starts.len());
    for (index, (&(at, before), next)) in starts.iter().zip(starts.iter().skip(1)).enumerate() {
        let x = run.x + before + gained + extra * index as f64;
        pieces.push(PaintLine {
            text: run.text[at..next.0].to_string(),
            x,
            y: run.y,
            width: next.1 - before + extra,
        });
    }
    let (at, before) = starts[starts.len() - 1];
    let open = marks.last() == Some(&run.text.len());
    let tail = run.width - before + if open { extra } else { 0.0 };
    pieces.push(PaintLine {
        text: run.text[at..].to_string(),
        x: run.x + before + gained + extra * (starts.len() - 1) as f64,
        y: run.y,
        width: tail.max(0.0),
    });
    let mut pieces = pieces
        .into_iter()
        .filter(|p| !p.text.is_empty() || p.width > 0.0);
    let lines = &mut items[item].lines;
    if let Some(first) = pieces.next() {
        lines[line] = first;
    }
    lines.extend(pieces);
    Ok(())
}

/** Each piece's start: its byte offset in the run and the shaped width of the text before it,
 * measured whole so kerning matches the run drawn as one string; the first starts at zero. */
fn prefixes(
    context: &mut LayoutContext<'_>,
    item: &DisplayItem,
    run: &PaintLine,
    marks: &[usize],
) -> EngineResult<Vec<(usize, f64)>> {
    let face = crate::platform::Face {
        size: item.font_size,
        weight: item.font_weight,
        family: item.font_family,
        spacing: item.letter_spacing,
        italic: item.italic,
        autospace: item.autospace,
    };
    let mut starts = vec![(0, 0.0)];
    for &at in marks.iter().filter(|at| **at < run.text.len()) {
        let head = &run.text[..at];
        let spaces = head.chars().filter(|c| widened(*c)).count() as f64;
        starts.push((
            at,
            context.measure(head, face)? + spaces * item.word_spacing,
        ));
    }
    Ok(starts)
}
