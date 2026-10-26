/**
 * Line-level bidirectional ordering (UAX #9, simplified to whole fragments): each fragment on a
 * finished line is strong right-to-left (Hebrew, Arabic and other RTL scripts), left-to-right
 * (other letters and digits, which in an RTL paragraph take the same even level as letters), or
 * neutral (spaces, punctuation, atomic inlines such as images, which count as U+FFFC). Neutrals
 * take the direction of the strong fragments around them when both agree, else the paragraph's
 * (rules N1 and N2); then rule L2 reverses every run at each level from the highest down, and
 * the fragments are laid out again in that visual order inside the line's extent.
 * ponytail: a fragment is one unit, so a run mixing scripts relies on the platform shaper for
 * its internal order; split runs at direction changes if a page needs finer ordering.
 */
use super::flow::DisplayItem;
use super::line::{Frag, Piece};

/** `unicode-bidi` keywords in the order of their `ComputedStyle::unicode_bidi` codes (0 is
`normal`). */
const MODES: [&str; 6] = [
    "normal",
    "embed",
    "isolate",
    "bidi-override",
    "isolate-override",
    "plaintext",
];
/** A `unicode-bidi` keyword's code. */
pub(super) fn mode(value: &str) -> Option<u8> {
    MODES
        .iter()
        .position(|mode| *mode == value)
        .map(|at| at as u8)
}
/** An element's text with the explicit formatting characters its `unicode-bidi` and
`direction` stand for (CSS Writing Modes 2.4.2): the platform shaper then embeds, overrides or
isolates it as UAX #9 does, mirroring brackets in an override. */
pub(super) fn controls<'a>(
    style: &crate::document::ComputedStyle,
    text: std::borrow::Cow<'a, str>,
) -> std::borrow::Cow<'a, str> {
    let rtl = style.rtl;
    // Empty text gets no controls (they would make an empty run its own fragment), and on a
    // block container `embed` and `isolate` do nothing (CSS Writing Modes 2.4.2).
    let block = !style.inline || style.inline_block;
    if text.trim().is_empty() || (block && matches!(style.unicode_bidi, 1 | 2)) {
        return text;
    }
    let (open, close) = match (style.unicode_bidi, rtl) {
        (1, false) => ("\u{202A}", "\u{202C}"),
        (1, true) => ("\u{202B}", "\u{202C}"),
        (2, false) => ("\u{2066}", "\u{2069}"),
        (2, true) => ("\u{2067}", "\u{2069}"),
        (3, false) => ("\u{202D}", "\u{202C}"),
        (3, true) => ("\u{202E}", "\u{202C}"),
        (4, false) => ("\u{2066}\u{202D}", "\u{202C}\u{2069}"),
        (4, true) => ("\u{2067}\u{202E}", "\u{202C}\u{2069}"),
        (5, _) => ("\u{2068}", "\u{2069}"),
        _ => return text,
    };
    format!("{open}{text}{close}").into()
}
/** The isolate initiators LRI, RLI and FSI. */
const ISOLATES: [char; 3] = ['\u{2066}', '\u{2067}', '\u{2068}'];

/** A fragment's resolved direction class. */
#[derive(Clone, Copy, PartialEq, Debug)]
enum Class {
    /// Strong left-to-right, or a number after one (rule W7).
    Left,
    /// Strong right-to-left.
    Right,
    /// A European number not after strong left-to-right text: it reads as right-to-left to
    /// the neutrals around it and sits two levels up (rules W7, N1 and I1).
    Number,
    /// Neither: takes its neighbours' direction.
    Neutral,
}

/** Lays one finished line's fragments out in visual order for a paragraph of direction `rtl`. */
pub(super) fn reorder(items: &mut [DisplayItem], frags: &[(Frag, f64)], rtl: bool) {
    let mut classes: Vec<Class> = frags.iter().map(|(frag, _)| class(items, frag)).collect();
    let mixed = classes.contains(&Class::Right);
    if frags.len() < 2 || !(rtl || mixed) {
        return;
    }
    resolve_numbers(&mut classes, rtl);
    mark_edges(items, frags, &classes, rtl);
    resolve_neutrals(&mut classes, rtl);
    let extents: Vec<(f64, f64)> = frags
        .iter()
        .map(|(frag, _)| super::line::extent(items, frag).unwrap_or((0.0, 0.0)))
        .collect();
    let units = units(&classes, &extents, rtl);
    let levels: Vec<u8> = units.iter().map(|unit| unit.level).collect();
    let order: Vec<Unit> = visual_order(&levels)
        .into_iter()
        .map(|at| units[at])
        .collect();
    place(items, frags, &extents, &order);
}

/** One piece of a line to order: a fragment, or the space between two (a word separator not
in either run's text), which is a neutral taking its level from its neighbours (rule N1). */
#[derive(Clone, Copy)]
struct Unit {
    /// Embedding level.
    level: u8,
    /// The fragment's index, or None for a gap.
    frag: Option<usize>,
    /// A gap's width.
    width: f64,
}

/** The line's fragments interleaved with the gaps between them, in logical order. */
fn units(classes: &[Class], extents: &[(f64, f64)], rtl: bool) -> Vec<Unit> {
    let paragraph = if rtl { Class::Right } else { Class::Left };
    let mut units = Vec::with_capacity(classes.len() * 2);
    for (at, class) in classes.iter().enumerate() {
        if at > 0 {
            let width = (extents[at].0 - extents[at - 1].1).max(0.0);
            let (before, after) = (strength(classes[at - 1]), strength(*class));
            let side = if before == after { before } else { paragraph };
            let level = level(side, rtl);
            units.push(Unit {
                level,
                frag: None,
                width,
            });
        }
        let level = level(*class, rtl);
        units.push(Unit {
            level,
            frag: Some(at),
            width: 0.0,
        });
    }
    units
}

/** The direction class of one fragment: the first strong character of its text. */
fn class(items: &[DisplayItem], frag: &Frag) -> Class {
    let Piece::Text { item, line } = frag.piece else {
        return Class::Neutral;
    };
    let Some(text) = items.get(item).and_then(|item| item.lines.get(line)) else {
        return Class::Neutral;
    };
    // An isolate is one neutral character to the text around it (UAX #9 rule X5).
    if text.text.starts_with(ISOLATES) {
        return Class::Neutral;
    }
    text.text
        .chars()
        .find_map(|c| match c {
            // An override or embedding sets the direction its text shows to the outside.
            '\u{202B}' | '\u{202E}' => Some(Class::Right),
            '\u{202A}' | '\u{202D}' => Some(Class::Left),
            _ if right_to_left(c) => Some(Class::Right),
            _ if c.is_ascii_digit() => Some(Class::Number),
            _ if c.is_alphanumeric() => Some(Class::Left),
            _ => None,
        })
        .unwrap_or(Class::Neutral)
}

/** Strong right-to-left characters: Hebrew, Arabic, Syriac, Thaana, NKo and their
presentation forms (bidi classes R and AL). */
fn right_to_left(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF)
}

/** A run is shaped alone in the paragraph's direction, so neutrals at its edges would resolve
against the paragraph instead of the strong text beside them on the line (`> b` after `a` in
an RTL paragraph mirrored to `<`). Where that neighbour's direction differs from the
paragraph's, a zero-width mark of it (LRM or RLM) goes on that edge, and the shaper resolves
the neutrals as the whole line does (rule N1). */
fn mark_edges(items: &mut [DisplayItem], frags: &[(Frag, f64)], classes: &[Class], rtl: bool) {
    let paragraph = if rtl { Class::Right } else { Class::Left };
    let strong = |at: &mut dyn Iterator<Item = &Class>| {
        at.map(|class| strength(*class))
            .find(|class| *class != Class::Neutral)
            .unwrap_or(paragraph)
    };
    for (at, (frag, _)) in frags.iter().enumerate() {
        let Piece::Text { item, line } = frag.piece else {
            continue;
        };
        let before = strong(&mut classes[..at].iter().rev());
        let after = strong(&mut classes[at + 1..].iter());
        let Some(run) = items
            .get_mut(item)
            .and_then(|item| item.lines.get_mut(line))
        else {
            continue;
        };
        if before != paragraph && run.text.starts_with(|c| !strong_char(c)) {
            run.text.insert(0, mark(before));
        }
        if after != paragraph && run.text.ends_with(|c| !strong_char(c)) {
            run.text.push(mark(after));
        }
    }
}

/** The zero-width mark of a direction. */
fn mark(class: Class) -> char {
    match class {
        Class::Left => '\u{200E}',
        _ => '\u{200F}',
    }
}

/** Whether a character has a strong direction (or is a digit, which N1 treats as one). */
fn strong_char(c: char) -> bool {
    right_to_left(c) || c.is_alphanumeric()
}

/** Rule W7: a number whose nearest preceding strong fragment (the paragraph at the line's
start) is left-to-right becomes left-to-right. */
fn resolve_numbers(classes: &mut [Class], rtl: bool) {
    let mut strong = if rtl { Class::Right } else { Class::Left };
    for class in classes.iter_mut() {
        match *class {
            Class::Left | Class::Right => strong = *class,
            Class::Number if strong == Class::Left => *class = Class::Left,
            _ => {}
        }
    }
}

/** Rules N1 and N2: a neutral between two strong fragments of one direction takes it, any
other neutral the paragraph's; the line's start and end count as the paragraph direction. */
fn resolve_neutrals(classes: &mut [Class], rtl: bool) {
    let paragraph = if rtl { Class::Right } else { Class::Left };
    let resolved = classes.to_vec();
    for (at, class) in classes.iter_mut().enumerate() {
        if *class != Class::Neutral {
            continue;
        }
        let before = resolved[..at].iter().rev().find(|c| **c != Class::Neutral);
        let after = resolved[at + 1..].iter().find(|c| **c != Class::Neutral);
        let (before, after) = (*before.unwrap_or(&paragraph), *after.unwrap_or(&paragraph));
        *class = if strength(before) == strength(after) {
            strength(before)
        } else {
            paragraph
        };
    }
}

/** The direction a class gives neutrals beside it (rule N1: numbers count as right-to-left). */
fn strength(class: Class) -> Class {
    match class {
        Class::Number => Class::Right,
        other => other,
    }
}

/** Embedding levels (rules I1 and I2): in a left-to-right paragraph right-to-left is 1, in a
right-to-left one it is 1 and left-to-right 2; a number is 2 in either. */
fn level(class: Class, rtl: bool) -> u8 {
    match (class, rtl) {
        (Class::Number, _) => 2,
        (Class::Right, _) => 1,
        (_, false) => 0,
        (_, true) => 2,
    }
}

/** Rule L2: from the highest level down to the lowest odd one, reverse every maximal run of
fragments at that level or above; returns logical indices in visual order. */
fn visual_order(levels: &[u8]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..levels.len()).collect();
    let highest = levels.iter().copied().max().unwrap_or(0);
    let lowest_odd = levels
        .iter()
        .copied()
        .filter(|l| l % 2 == 1)
        .min()
        .unwrap_or(1);
    for level in (lowest_odd..=highest).rev() {
        let mut at = 0;
        while at < order.len() {
            if levels[order[at]] < level {
                at += 1;
                continue;
            }
            let end = (at..order.len())
                .find(|&i| levels[order[i]] < level)
                .unwrap_or(order.len());
            order[at..end].reverse();
            at = end;
        }
    }
    order
}

/** Moves each fragment to its visual slot: from the line's start edge, in visual order, each
fragment keeps its width and each gap its own, wherever its level put it. */
fn place(items: &mut [DisplayItem], frags: &[(Frag, f64)], extents: &[(f64, f64)], order: &[Unit]) {
    let mut cursor = extents.iter().map(|e| e.0).fold(f64::INFINITY, f64::min);
    for unit in order {
        let Some(index) = unit.frag else {
            cursor += unit.width;
            continue;
        };
        let dx = cursor - extents[index].0;
        super::line::shift(items, frags[index].0.piece, dx);
        cursor += extents[index].1 - extents[index].0;
    }
}

#[cfg(test)]
mod tests {
    use super::visual_order;

    /** Rule L2 on an RTL paragraph: left-to-right runs keep their order, the rest reverses. */
    #[test]
    fn left_to_right_runs_keep_their_order_inside_a_right_to_left_line() {
        assert_eq!(visual_order(&[1, 2, 2, 1]), vec![3, 1, 2, 0]);
        assert_eq!(visual_order(&[0, 1, 1, 0]), vec![0, 2, 1, 3]);
    }
}
