/**
 * UAX #14's prohibited breaks around a boundary layout otherwise treats as breakable: between a
 * run and an atomic inline (which Blink breaks around as U+FFFC) and between runs of different
 * elements. Glue (class GL: the no-break spaces and hyphen, the Tibetan marks), word joiners
 * (WJ) and the zero width joiner forbid a break after them (LB11, LB12, LB8a); the same glue,
 * joiners and combining marks forbid one before them (LB11, LB12a, LB9).
 */
use super::flow::*;

/** Class GL, WJ and ZWJ scalars: no line breaks after any of them. The no-break space is left
 * out, as Blink breaks between it and an atomic inline. */
const GLUE: [char; 12] = [
    '\u{202f}', '\u{2007}', '\u{2011}', '\u{f08}', '\u{f0c}', '\u{f12}', '\u{34f}', '\u{180e}',
    '\u{2060}', '\u{feff}', '\u{200d}', '\u{35c}',
];

/** Whether no break may follow `c`. */
fn holds_after(c: char) -> bool {
    GLUE.contains(&c)
}

/** Whether no break may precede `c`: glue, or a combining mark extending what comes before. */
fn holds_before(c: char) -> bool {
    GLUE.contains(&c) || matches!(c as u32, 0x0300..=0x036f | 0x20d0..=0x20ff | 0xfe20..=0xfe2f)
}

/** Whether the line's content so far ends in glue with no space after it, so whatever is
 * placed next (an atomic inline, or another element's text) stays on this line. */
pub(super) fn held(flow: &Flow) -> bool {
    flow.space != Some(true) && flow.last.is_some_and(holds_after)
}

/** Whether a run that starts with `text`, placed with no space before it, must stay on the line
 * it follows: it opens with glue or a combining mark, the line ends in glue, or UAX #14 finds no
 * break between the line's last scalar and the run's first (letters across `a<b>b</b>`, or
 * an ellipsis after an ideograph in another element). Where the joined run would not fit,
 * `lookahead` has already broken the earlier word so that it does. */
pub(super) fn joined(flow: &Flow, text: &str, breaks: super::wrap::Breaks) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    holds_before(first)
        || held(flow)
        || (!flow.last_fill
            && flow
                .last
                .is_some_and(|last| !super::wrap::between(last, first, breaks)))
}
